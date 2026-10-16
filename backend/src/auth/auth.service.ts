import {
  Injectable,
  UnauthorizedException,
  ConflictException,
} from '@nestjs/common';
import { JwtService } from '@nestjs/jwt';
import { InjectRepository } from '@nestjs/typeorm';
import { Repository, Not, IsNull } from 'typeorm';
import * as bcrypt from 'bcrypt';
import * as crypto from 'crypto';
import { Merchant, MerchantStatus } from '../merchants/entities/merchant.entity';
import { RefreshToken } from './refresh-token.entity';
import { RegisterDto } from './dto/register.dto';
import { LoginDto } from './dto/login.dto';
import { AuthTokenResponseDto } from './dto/auth-token-response.dto';

/** Lifetime of a refresh token: 30 days. */
const REFRESH_TOKEN_TTL_MS = 30 * 24 * 60 * 60 * 1000;

@Injectable()
export class AuthService {
  constructor(
    @InjectRepository(Merchant)
    private readonly merchantsRepo: Repository<Merchant>,
    @InjectRepository(RefreshToken)
    private readonly refreshTokenRepo: Repository<RefreshToken>,
    private readonly jwtService: JwtService,
  ) {}

  async register(dto: RegisterDto): Promise<AuthTokenResponseDto> {
    const existing = await this.merchantsRepo.findOne({ where: { email: dto.email } });
    if (existing) throw new ConflictException('Email already registered');

    const passwordHash = await bcrypt.hash(dto.password, 12);
    const merchant = this.merchantsRepo.create({
      email: dto.email,
      passwordHash,
      businessName: dto.businessName,
      businessType: dto.businessType,
      country: dto.country,
      status: MerchantStatus.ACTIVE,
    });

    const saved = await this.merchantsRepo.save(merchant);
    return this.issueTokenPair(saved);
  }

  async login(dto: LoginDto): Promise<AuthTokenResponseDto> {
    const merchant = await this.merchantsRepo.findOne({ where: { email: dto.email } });
    if (!merchant) throw new UnauthorizedException('Invalid credentials');

    const valid = await bcrypt.compare(dto.password, merchant.passwordHash);
    if (!valid) throw new UnauthorizedException('Invalid credentials');

    return this.issueTokenPair(merchant);
  }

  /**
   * Rotate a refresh token.
   *
   * Flow:
   *  1. SHA-256 hash the incoming opaque token.
   *  2. Look up the stored record by hash.
   *  3. Reject if not found (already rotated / never issued) — 401.
   *  4. Reject if expired — 401.
   *  5. Delete the old token record (single-use rotation).
   *  6. Issue and return a fresh token pair.
   */
  async refresh(rawToken: string): Promise<AuthTokenResponseDto> {
    const tokenHash = this.hashToken(rawToken);

    const stored = await this.refreshTokenRepo.findOne({
      where: { tokenHash },
      relations: ['merchant'],
    });

    if (!stored) {
      throw new UnauthorizedException('Invalid or already-used refresh token');
    }

    if (stored.expiresAt < new Date()) {
      // Clean up the expired record and reject.
      await this.refreshTokenRepo.delete({ id: stored.id });
      throw new UnauthorizedException('Refresh token expired');
    }

    const merchant = stored.merchant;

    // Invalidate the used token before issuing a new pair (rotation).
    await this.refreshTokenRepo.delete({ id: stored.id });

    return this.issueTokenPair(merchant);
  }

  /** Timing-safe API key lookup — iterates all merchants and uses bcrypt.compare */
  async findMerchantByApiKey(rawKey: string): Promise<Merchant | null> {
    const merchants = await this.merchantsRepo.find({
      where: { apiKeyHash: Not(IsNull()) },
      withDeleted: false,
    });
    for (const m of merchants) {
      if (m.apiKeyHash && (await bcrypt.compare(rawKey, m.apiKeyHash))) {
        if (m.status !== MerchantStatus.ACTIVE || m.deletedAt) {
          throw new UnauthorizedException('Merchant account is suspended or inactive');
        }
        return m;
      }
    }
    return null;
  }

  // ---------------------------------------------------------------------------
  // Private helpers
  // ---------------------------------------------------------------------------

  /**
   * Generate a fresh access + refresh token pair, persist the hashed refresh
   * token, and return the full response DTO.
   */
  private async issueTokenPair(merchant: Merchant): Promise<AuthTokenResponseDto> {
    const accessToken = this.signToken(merchant.id, merchant.email, merchant.role);

    // Generate a cryptographically random opaque refresh token.
    const rawRefreshToken = crypto.randomBytes(40).toString('hex');
    const tokenHash = this.hashToken(rawRefreshToken);

    const record = this.refreshTokenRepo.create({
      tokenHash,
      merchantId: merchant.id,
      merchant,
      expiresAt: new Date(Date.now() + REFRESH_TOKEN_TTL_MS),
    });
    await this.refreshTokenRepo.save(record);

    const { passwordHash: _, apiKeyHash: __, ...merchantData } = merchant as any;
    return { accessToken, refreshToken: rawRefreshToken, merchant: merchantData };
  }

  /** SHA-256 hex digest of a raw token string. */
  private hashToken(raw: string): string {
    return crypto.createHash('sha256').update(raw).digest('hex');
  }

  private signToken(sub: string, email: string, role?: string): string {
    return this.jwtService.sign({ sub, email, role });
  }
}
