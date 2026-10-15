import { instanceToPlain } from 'class-transformer';
import { Merchant, MerchantRole, MerchantStatus } from './merchant.entity';

describe('Merchant entity — serialization (issue #30)', () => {
  function buildMerchant(): Merchant {
    return Object.assign(new Merchant(), {
      id: 'uuid-1',
      email: 'test@example.com',
      passwordHash: 'bcrypt-hash',
      businessName: 'Test Corp',
      businessType: 'retail',
      country: 'NG',
      status: MerchantStatus.ACTIVE,
      role: MerchantRole.MERCHANT,
      apiKeyHash: 'api-key-hash',
      feeRate: 0.015,
      sandboxMode: false,
      createdAt: new Date('2024-01-01'),
      updatedAt: new Date('2024-01-01'),
      deletedAt: null,
    });
  }

  it('instanceToPlain excludes passwordHash', () => {
    const plain = instanceToPlain(buildMerchant());
    expect(plain.passwordHash).toBeUndefined();
  });

  it('instanceToPlain excludes apiKeyHash', () => {
    const plain = instanceToPlain(buildMerchant());
    expect(plain.apiKeyHash).toBeUndefined();
  });

  it('raw apiKey column no longer exists on the entity class', () => {
    const merchant = buildMerchant();
    // The property should not exist at all — the column was removed in #30.
    expect(Object.prototype.hasOwnProperty.call(merchant, 'apiKey')).toBe(false);
    expect((merchant as any).apiKey).toBeUndefined();
  });

  it('instanceToPlain includes non-sensitive public fields', () => {
    const plain = instanceToPlain(buildMerchant());
    expect(plain.id).toBe('uuid-1');
    expect(plain.email).toBe('test@example.com');
    expect(plain.businessName).toBe('Test Corp');
    expect(plain.status).toBe(MerchantStatus.ACTIVE);
    expect(plain.role).toBe(MerchantRole.MERCHANT);
  });
});
