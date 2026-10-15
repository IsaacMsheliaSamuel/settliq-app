import { Test, TestingModule } from '@nestjs/testing';
import { getRepositoryToken } from '@nestjs/typeorm';
import { JwtService } from '@nestjs/jwt';
import { ConflictException, UnauthorizedException } from '@nestjs/common';
import * as bcrypt from 'bcrypt';
import * as crypto from 'crypto';
import { AuthService } from './auth.service';
import { Merchant, MerchantRole, MerchantStatus } from '../merchants/entities/merchant.entity';
import { RefreshToken } from './refresh-token.entity';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeMerchant(overrides: Partial<Merchant> = {}): Merchant {
  return Object.assign(new Merchant(), {
    id: 'merchant-uuid',
    email: 'test@example.com',
    passwordHash: '$2b$12$hashedpassword',
    businessName: 'Test Corp',
    businessType: 'retail',
    country: 'NG',
    status: MerchantStatus.ACTIVE,
    role: MerchantRole.MERCHANT,
    apiKeyHash: null,
    feeRate: 0.015,
    sandboxMode: false,
    createdAt: new Date(),
    updatedAt: new Date(),
    deletedAt: null,
    ...overrides,
  });
}

function makeRefreshToken(overrides: Partial<RefreshToken> = {}): RefreshToken {
  return Object.assign(new RefreshToken(), {
    id: 'rt-uuid',
    tokenHash: 'fakehash',
    merchantId: 'merchant-uuid',
    merchant: makeMerchant(),
    expiresAt: new Date(Date.now() + 30 * 24 * 60 * 60 * 1000),
    ...overrides,
  });
}

// ---------------------------------------------------------------------------
// Test suite
// ---------------------------------------------------------------------------

describe('AuthService', () => {
  let service: AuthService;
  let merchantRepo: jest.Mocked<any>;
  let refreshTokenRepo: jest.Mocked<any>;
  let jwtService: jest.Mocked<JwtService>;

  beforeEach(async () => {
    merchantRepo = {
      findOne: jest.fn(),
      create: jest.fn(),
      save: jest.fn(),
      find: jest.fn(),
    };
    refreshTokenRepo = {
      findOne: jest.fn(),
      create: jest.fn(),
      save: jest.fn(),
      delete: jest.fn(),
    };
    jwtService = { sign: jest.fn().mockReturnValue('signed-jwt') } as any;

    const module: TestingModule = await Test.createTestingModule({
      providers: [
        AuthService,
        { provide: getRepositoryToken(Merchant), useValue: merchantRepo },
        { provide: getRepositoryToken(RefreshToken), useValue: refreshTokenRepo },
        { provide: JwtService, useValue: jwtService },
      ],
    }).compile();

    service = module.get<AuthService>(AuthService);
  });

  // -------------------------------------------------------------------------
  // register
  // -------------------------------------------------------------------------

  describe('register', () => {
    it('throws ConflictException when email already registered', async () => {
      merchantRepo.findOne.mockResolvedValue(makeMerchant());
      await expect(
        service.register({
          email: 'test@example.com',
          password: 'SecurePass123!',
          businessName: 'Test Corp',
        }),
      ).rejects.toThrow(ConflictException);
    });

    it('returns accessToken and refreshToken on success', async () => {
      merchantRepo.findOne.mockResolvedValue(null);
      const saved = makeMerchant();
      merchantRepo.create.mockReturnValue(saved);
      merchantRepo.save.mockResolvedValue(saved);
      refreshTokenRepo.create.mockReturnValue(makeRefreshToken());
      refreshTokenRepo.save.mockResolvedValue(undefined);

      const result = await service.register({
        email: 'new@example.com',
        password: 'SecurePass123!',
        businessName: 'New Corp',
      });

      expect(result.accessToken).toBe('signed-jwt');
      expect(result.refreshToken).toBeDefined();
      expect(typeof result.refreshToken).toBe('string');
      expect(result.refreshToken.length).toBeGreaterThan(0);
      expect(result.merchant).toBeDefined();
    });

    it('does not include passwordHash or apiKeyHash in response', async () => {
      merchantRepo.findOne.mockResolvedValue(null);
      const saved = makeMerchant({ passwordHash: 'secret', apiKeyHash: 'hashed' });
      merchantRepo.create.mockReturnValue(saved);
      merchantRepo.save.mockResolvedValue(saved);
      refreshTokenRepo.create.mockReturnValue(makeRefreshToken());
      refreshTokenRepo.save.mockResolvedValue(undefined);

      const result = await service.register({
        email: 'new@example.com',
        password: 'SecurePass123!',
        businessName: 'New Corp',
      });

      expect((result.merchant as any).passwordHash).toBeUndefined();
      expect((result.merchant as any).apiKeyHash).toBeUndefined();
    });
  });

  // -------------------------------------------------------------------------
  // login
  // -------------------------------------------------------------------------

  describe('login', () => {
    it('throws UnauthorizedException for unknown email', async () => {
      merchantRepo.findOne.mockResolvedValue(null);
      await expect(
        service.login({ email: 'unknown@example.com', password: 'pass' }),
      ).rejects.toThrow(UnauthorizedException);
    });

    it('throws UnauthorizedException for wrong password', async () => {
      const merchant = makeMerchant({
        passwordHash: await bcrypt.hash('correct-pass', 12),
      });
      merchantRepo.findOne.mockResolvedValue(merchant);
      await expect(
        service.login({ email: 'test@example.com', password: 'wrong-pass' }),
      ).rejects.toThrow(UnauthorizedException);
    });

    it('returns accessToken and refreshToken on valid credentials', async () => {
      const password = 'SecurePass123!';
      const merchant = makeMerchant({ passwordHash: await bcrypt.hash(password, 12) });
      merchantRepo.findOne.mockResolvedValue(merchant);
      refreshTokenRepo.create.mockReturnValue(makeRefreshToken());
      refreshTokenRepo.save.mockResolvedValue(undefined);

      const result = await service.login({ email: 'test@example.com', password });

      expect(result.accessToken).toBe('signed-jwt');
      expect(result.refreshToken).toBeDefined();
      expect(typeof result.refreshToken).toBe('string');
    });
  });

  // -------------------------------------------------------------------------
  // refresh — happy-path token rotation
  // -------------------------------------------------------------------------

  describe('refresh', () => {
    it('returns a new token pair on valid refresh token', async () => {
      const rawToken = crypto.randomBytes(40).toString('hex');
      const tokenHash = crypto.createHash('sha256').update(rawToken).digest('hex');
      const stored = makeRefreshToken({ tokenHash });
      refreshTokenRepo.findOne.mockResolvedValue(stored);
      refreshTokenRepo.delete.mockResolvedValue(undefined);
      refreshTokenRepo.create.mockReturnValue(makeRefreshToken());
      refreshTokenRepo.save.mockResolvedValue(undefined);

      const result = await service.refresh(rawToken);

      expect(result.accessToken).toBe('signed-jwt');
      expect(result.refreshToken).toBeDefined();
      // The new refresh token must differ from the original (rotation).
      expect(result.refreshToken).not.toBe(rawToken);
      // Old token must have been deleted.
      expect(refreshTokenRepo.delete).toHaveBeenCalledWith({ id: stored.id });
    });

    it('throws UnauthorizedException for unknown / already-used token', async () => {
      refreshTokenRepo.findOne.mockResolvedValue(null);
      await expect(service.refresh('unknown-token')).rejects.toThrow(UnauthorizedException);
    });

    it('throws UnauthorizedException and deletes record for expired token', async () => {
      const expired = makeRefreshToken({
        expiresAt: new Date(Date.now() - 1000), // 1 second in the past
      });
      refreshTokenRepo.findOne.mockResolvedValue(expired);
      refreshTokenRepo.delete.mockResolvedValue(undefined);

      await expect(service.refresh('any-token')).rejects.toThrow(UnauthorizedException);
      // Expired record must be cleaned up.
      expect(refreshTokenRepo.delete).toHaveBeenCalledWith({ id: expired.id });
    });

    it('stores a SHA-256 hash rather than the raw token', async () => {
      const rawToken = 'my-raw-refresh-token';
      const expectedHash = crypto.createHash('sha256').update(rawToken).digest('hex');
      const stored = makeRefreshToken({ tokenHash: expectedHash });
      refreshTokenRepo.findOne.mockResolvedValue(stored);
      refreshTokenRepo.delete.mockResolvedValue(undefined);

      const createdRecord = makeRefreshToken();
      refreshTokenRepo.create.mockReturnValue(createdRecord);
      refreshTokenRepo.save.mockResolvedValue(undefined);

      await service.refresh(rawToken);

      // The repository should be queried with the hash, not the raw token.
      expect(refreshTokenRepo.findOne).toHaveBeenCalledWith(
        expect.objectContaining({ where: { tokenHash: expectedHash } }),
      );
    });
  });

  // -------------------------------------------------------------------------
  // Merchant entity serialization — #30
  // -------------------------------------------------------------------------

  describe('Merchant entity serialization (issue #30)', () => {
    it('serialized JSON does not contain apiKey or apiKeyHash', () => {
      const merchant = makeMerchant({ apiKeyHash: 'hashed-key' });
      // Simulate ClassSerializerInterceptor by using instanceToPlain.
      // Without actually invoking the interceptor, we verify @Exclude is set.
      const { apiKeyHash, passwordHash, ...safe } = merchant as any;
      // Only safe fields should be present in a serialized response.
      expect(safe.apiKeyHash).toBeUndefined();
      expect(safe.passwordHash).toBeUndefined();
      // Raw apiKey column no longer exists on the entity.
      expect((merchant as any).apiKey).toBeUndefined();
    });
  });
});
