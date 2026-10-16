import { Test, TestingModule } from '@nestjs/testing';
import { getRepositoryToken } from '@nestjs/typeorm';
import { JwtService } from '@nestjs/jwt';
import { ConflictException, UnauthorizedException } from '@nestjs/common';
import { Repository } from 'typeorm';
import * as bcrypt from 'bcrypt';
import { AuthService } from './auth.service';
import { Merchant, MerchantStatus, MerchantRole } from '../merchants/entities/merchant.entity';

// ── Helpers ────────────────────────────────────────────────────────────────

function makeMerchant(overrides: Partial<Merchant> = {}): Merchant {
  return Object.assign(new Merchant(), {
    id: 'uuid-1',
    email: 'merchant@example.com',
    passwordHash: '$2b$12$hashedpassword',
    businessName: 'Acme Corp',
    businessType: 'retail',
    country: 'NG',
    status: MerchantStatus.ACTIVE,
    role: MerchantRole.MERCHANT,
    apiKey: null,
    apiKeyHash: null,
    feeRate: 0.015,
    sandboxMode: false,
    createdAt: new Date('2024-01-01'),
    updatedAt: new Date('2024-01-01'),
    deletedAt: null,
    ...overrides,
  });
}

function makeRepo(): jest.Mocked<
  Pick<Repository<Merchant>, 'findOne' | 'create' | 'save' | 'find' | 'increment'>
> {
  return {
    findOne: jest.fn(),
    create: jest.fn(),
    save: jest.fn(),
    find: jest.fn(),
    increment: jest.fn(),
  };
}

// ── Tests ──────────────────────────────────────────────────────────────────

describe('AuthService', () => {
  let service: AuthService;
  let repo: ReturnType<typeof makeRepo>;
  let jwtService: jest.Mocked<Pick<JwtService, 'sign'>>;

  beforeEach(async () => {
    repo = makeRepo();
    jwtService = { sign: jest.fn().mockReturnValue('signed-token') };

    const module: TestingModule = await Test.createTestingModule({
      providers: [
        AuthService,
        { provide: getRepositoryToken(Merchant), useValue: repo },
        { provide: JwtService, useValue: jwtService },
      ],
    }).compile();

    service = module.get<AuthService>(AuthService);
  });

  // ── register ──────────────────────────────────────────────────────────

  describe('register', () => {
    const dto = {
      email: 'new@example.com',
      password: 'SecurePass123!',
      businessName: 'NewCo',
      businessType: 'saas',
      country: 'US',
    };

    it('hashes the password with bcrypt before saving', async () => {
      repo.findOne.mockResolvedValue(null);
      const merchant = makeMerchant({ email: dto.email, id: 'new-id' });
      repo.create.mockReturnValue(merchant);
      repo.save.mockResolvedValue(merchant);

      await service.register(dto);

      const savedArg = repo.create.mock.calls[0][0] as Partial<Merchant>;
      expect(savedArg.passwordHash).toBeDefined();
      expect(savedArg.passwordHash).not.toEqual(dto.password);
      const isHashed = await bcrypt.compare(dto.password, savedArg.passwordHash!);
      expect(isHashed).toBe(true);
    });

    it('returns an accessToken and merchant data (without sensitive fields)', async () => {
      repo.findOne.mockResolvedValue(null);
      const merchant = makeMerchant({ email: dto.email, id: 'new-id' });
      repo.create.mockReturnValue(merchant);
      repo.save.mockResolvedValue(merchant);

      const result = await service.register(dto);

      expect(result.accessToken).toBe('signed-token');
      expect(result.merchant).not.toHaveProperty('passwordHash');
      expect(result.merchant).not.toHaveProperty('apiKeyHash');
    });

    it('throws ConflictException when email is already registered', async () => {
      repo.findOne.mockResolvedValue(makeMerchant());

      await expect(service.register(dto)).rejects.toThrow(ConflictException);
    });

    it('signs the JWT with the saved merchant id, email, and role', async () => {
      repo.findOne.mockResolvedValue(null);
      const merchant = makeMerchant({ id: 'new-id', email: dto.email, role: MerchantRole.MERCHANT });
      repo.create.mockReturnValue(merchant);
      repo.save.mockResolvedValue(merchant);

      await service.register(dto);

      expect(jwtService.sign).toHaveBeenCalledWith({
        sub: 'new-id',
        email: dto.email,
        role: MerchantRole.MERCHANT,
      });
    });
  });

  // ── login ──────────────────────────────────────────────────────────────

  describe('login', () => {
    it('returns accessToken and merchant data when credentials are valid', async () => {
      const password = 'SecurePass123!';
      const passwordHash = await bcrypt.hash(password, 12);
      const merchant = makeMerchant({ passwordHash });
      repo.findOne.mockResolvedValue(merchant);

      const result = await service.login({ email: merchant.email, password });

      expect(result.accessToken).toBe('signed-token');
      expect(result.merchant).not.toHaveProperty('passwordHash');
    });

    it('throws UnauthorizedException for unknown email', async () => {
      repo.findOne.mockResolvedValue(null);

      await expect(
        service.login({ email: 'ghost@example.com', password: 'pass' }),
      ).rejects.toThrow(UnauthorizedException);
    });

    it('throws UnauthorizedException for incorrect password', async () => {
      const passwordHash = await bcrypt.hash('correct-password', 12);
      repo.findOne.mockResolvedValue(makeMerchant({ passwordHash }));

      await expect(
        service.login({ email: 'merchant@example.com', password: 'wrong-password' }),
      ).rejects.toThrow(UnauthorizedException);
    });

    it('uses a constant error message for both missing user and wrong password (no oracle)', async () => {
      repo.findOne.mockResolvedValue(null);
      let err1: Error | undefined;
      try {
        await service.login({ email: 'ghost@example.com', password: 'x' });
      } catch (e) {
        err1 = e as Error;
      }

      const passwordHash = await bcrypt.hash('real', 12);
      repo.findOne.mockResolvedValue(makeMerchant({ passwordHash }));
      let err2: Error | undefined;
      try {
        await service.login({ email: 'merchant@example.com', password: 'wrong' });
      } catch (e) {
        err2 = e as Error;
      }

      expect(err1?.message).toBe(err2?.message);
    });
  });

  // ── findMerchantByApiKey ───────────────────────────────────────────────

  describe('findMerchantByApiKey', () => {
    it('returns merchant when the raw key matches one of the hashed keys', async () => {
      const rawKey = 'my-raw-api-key';
      const apiKeyHash = await bcrypt.hash(rawKey, 12);
      const merchant = makeMerchant({ apiKeyHash });
      repo.find.mockResolvedValue([merchant]);

      const result = await service.findMerchantByApiKey(rawKey);

      expect(result).toBe(merchant);
    });

    it('returns null when no key matches', async () => {
      const apiKeyHash = await bcrypt.hash('other-key', 12);
      repo.find.mockResolvedValue([makeMerchant({ apiKeyHash })]);

      const result = await service.findMerchantByApiKey('wrong-key');

      expect(result).toBeNull();
    });

    it('returns null when there are no merchants with an apiKeyHash', async () => {
      repo.find.mockResolvedValue([]);

      const result = await service.findMerchantByApiKey('any-key');

      expect(result).toBeNull();
    });
  });
});
