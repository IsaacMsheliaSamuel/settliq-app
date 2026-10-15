import { Test, TestingModule } from '@nestjs/testing';
import { getRepositoryToken } from '@nestjs/typeorm';
import { JwtService } from '@nestjs/jwt';
import { UnauthorizedException, ConflictException } from '@nestjs/common';
import * as bcrypt from 'bcrypt';
import { AuthService } from './auth.service';
import { Merchant, MerchantStatus, MerchantRole } from '../merchants/entities/merchant.entity';

const mockMerchant = (overrides: Partial<Merchant> = {}): Merchant =>
  ({
    id: 'uuid-123',
    email: 'test@example.com',
    passwordHash: '$2b$12$hashed',
    businessName: 'Test Corp',
    businessType: 'retail',
    country: 'US',
    status: MerchantStatus.ACTIVE,
    role: MerchantRole.MERCHANT,
    apiKey: null,
    apiKeyHash: '$2b$12$apikeyhash',
    feeRate: 0.015,
    sandboxMode: false,
    createdAt: new Date(),
    updatedAt: new Date(),
    deletedAt: null,
    ...overrides,
  } as Merchant);

describe('AuthService', () => {
  let service: AuthService;
  let merchantsRepo: {
    findOne: jest.Mock;
    find: jest.Mock;
    create: jest.Mock;
    save: jest.Mock;
  };
  let jwtService: { sign: jest.Mock };

  beforeEach(async () => {
    merchantsRepo = {
      findOne: jest.fn(),
      find: jest.fn(),
      create: jest.fn(),
      save: jest.fn(),
    };
    jwtService = { sign: jest.fn().mockReturnValue('signed-token') };

    const module: TestingModule = await Test.createTestingModule({
      providers: [
        AuthService,
        { provide: getRepositoryToken(Merchant), useValue: merchantsRepo },
        { provide: JwtService, useValue: jwtService },
      ],
    }).compile();

    service = module.get<AuthService>(AuthService);
  });

  describe('findMerchantByApiKey', () => {
    it('returns the merchant when key matches an active merchant', async () => {
      const merchant = mockMerchant({ status: MerchantStatus.ACTIVE, deletedAt: null });
      merchantsRepo.find.mockResolvedValue([merchant]);
      jest.spyOn(bcrypt, 'compare').mockResolvedValue(true as never);

      const result = await service.findMerchantByApiKey('raw-api-key');

      expect(result).toBe(merchant);
    });

    it('returns null when no merchant matches the raw key', async () => {
      const merchant = mockMerchant();
      merchantsRepo.find.mockResolvedValue([merchant]);
      jest.spyOn(bcrypt, 'compare').mockResolvedValue(false as never);

      const result = await service.findMerchantByApiKey('wrong-key');

      expect(result).toBeNull();
    });

    it('throws UnauthorizedException for a suspended merchant using a valid API key (#33)', async () => {
      const suspended = mockMerchant({ status: MerchantStatus.SUSPENDED, deletedAt: null });
      merchantsRepo.find.mockResolvedValue([suspended]);
      jest.spyOn(bcrypt, 'compare').mockResolvedValue(true as never);

      await expect(service.findMerchantByApiKey('raw-api-key')).rejects.toThrow(
        UnauthorizedException,
      );
    });

    it('throws UnauthorizedException for a pending merchant using a valid API key (#33)', async () => {
      const pending = mockMerchant({ status: MerchantStatus.PENDING, deletedAt: null });
      merchantsRepo.find.mockResolvedValue([pending]);
      jest.spyOn(bcrypt, 'compare').mockResolvedValue(true as never);

      await expect(service.findMerchantByApiKey('raw-api-key')).rejects.toThrow(
        UnauthorizedException,
      );
    });

    it('throws UnauthorizedException for a soft-deleted merchant using a valid API key (#33)', async () => {
      const deleted = mockMerchant({
        status: MerchantStatus.ACTIVE,
        deletedAt: new Date('2024-01-01'),
      });
      merchantsRepo.find.mockResolvedValue([deleted]);
      jest.spyOn(bcrypt, 'compare').mockResolvedValue(true as never);

      await expect(service.findMerchantByApiKey('raw-api-key')).rejects.toThrow(
        UnauthorizedException,
      );
    });

    it('throws with message "Merchant account is suspended or inactive" (#33)', async () => {
      const suspended = mockMerchant({ status: MerchantStatus.SUSPENDED });
      merchantsRepo.find.mockResolvedValue([suspended]);
      jest.spyOn(bcrypt, 'compare').mockResolvedValue(true as never);

      await expect(service.findMerchantByApiKey('raw-api-key')).rejects.toThrow(
        'Merchant account is suspended or inactive',
      );
    });

    it('passes withDeleted: false to exclude soft-deleted rows at query level (#33)', async () => {
      merchantsRepo.find.mockResolvedValue([]);

      await service.findMerchantByApiKey('any-key');

      expect(merchantsRepo.find).toHaveBeenCalledWith(
        expect.objectContaining({ withDeleted: false }),
      );
    });
  });

  describe('login', () => {
    it('throws UnauthorizedException for unknown email', async () => {
      merchantsRepo.findOne.mockResolvedValue(null);

      await expect(service.login({ email: 'none@example.com', password: 'pw' })).rejects.toThrow(
        UnauthorizedException,
      );
    });

    it('throws UnauthorizedException for wrong password', async () => {
      const merchant = mockMerchant();
      merchantsRepo.findOne.mockResolvedValue(merchant);
      jest.spyOn(bcrypt, 'compare').mockResolvedValue(false as never);

      await expect(
        service.login({ email: merchant.email, password: 'wrong' }),
      ).rejects.toThrow(UnauthorizedException);
    });
  });

  describe('register', () => {
    it('throws ConflictException when email already exists', async () => {
      merchantsRepo.findOne.mockResolvedValue(mockMerchant());

      await expect(
        service.register({
          email: 'test@example.com',
          password: 'pass',
          businessName: 'X',
          businessType: 'retail',
          country: 'US',
        }),
      ).rejects.toThrow(ConflictException);
    });
  });
});
