import { Test, TestingModule } from '@nestjs/testing';
import { AuthController } from './auth.controller';
import { AuthService } from './auth.service';
import { ThrottlerModule } from '@nestjs/throttler';
import { MerchantRole, MerchantStatus } from '../merchants/entities/merchant.entity';

// ── Helpers ────────────────────────────────────────────────────────────────

function makeTokenResponse() {
  return {
    accessToken: 'signed-token',
    merchant: {
      id: 'uuid-1',
      email: 'merchant@example.com',
      businessName: 'Acme Corp',
      businessType: 'retail',
      country: 'NG',
      status: MerchantStatus.ACTIVE,
      role: MerchantRole.MERCHANT,
      apiKey: null,
      feeRate: 0.015,
      sandboxMode: false,
      createdAt: new Date('2024-01-01'),
      updatedAt: new Date('2024-01-01'),
      deletedAt: null,
    },
  };
}

// ── Tests ──────────────────────────────────────────────────────────────────

describe('AuthController', () => {
  let controller: AuthController;
  let authService: jest.Mocked<Pick<AuthService, 'register' | 'login'>>;

  beforeEach(async () => {
    authService = {
      register: jest.fn(),
      login: jest.fn(),
    };

    const module: TestingModule = await Test.createTestingModule({
      imports: [ThrottlerModule.forRoot([{ ttl: 60000, limit: 60 }])],
      controllers: [AuthController],
      providers: [{ provide: AuthService, useValue: authService }],
    }).compile();

    controller = module.get<AuthController>(AuthController);
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

    it('delegates to authService.register and returns the result', async () => {
      const expected = makeTokenResponse();
      authService.register.mockResolvedValue(expected);

      const result = await controller.register(dto);

      expect(authService.register).toHaveBeenCalledWith(dto);
      expect(result).toBe(expected);
    });

    it('propagates errors thrown by authService.register', async () => {
      authService.register.mockRejectedValue(new Error('conflict'));

      await expect(controller.register(dto)).rejects.toThrow('conflict');
    });
  });

  // ── login ──────────────────────────────────────────────────────────────

  describe('login', () => {
    const dto = { email: 'merchant@example.com', password: 'SecurePass123!' };

    it('delegates to authService.login and returns the result', async () => {
      const expected = makeTokenResponse();
      authService.login.mockResolvedValue(expected);

      const result = await controller.login(dto);

      expect(authService.login).toHaveBeenCalledWith(dto);
      expect(result).toBe(expected);
    });

    it('propagates errors thrown by authService.login', async () => {
      authService.login.mockRejectedValue(new Error('unauthorized'));

      await expect(controller.login(dto)).rejects.toThrow('unauthorized');
    });
  });
});
