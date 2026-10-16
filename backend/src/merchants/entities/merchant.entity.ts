import {
  Entity,
  PrimaryGeneratedColumn,
  Column,
  CreateDateColumn,
  UpdateDateColumn,
  DeleteDateColumn,
  Index,
} from 'typeorm';
import { Exclude } from 'class-transformer';

export enum MerchantStatus {
  ACTIVE = 'active',
  SUSPENDED = 'suspended',
  PENDING = 'pending',
}

export enum MerchantRole {
  ADMIN = 'admin',
  MERCHANT = 'merchant',
  SUPERADMIN = 'superadmin',
}

@Entity('merchants')
export class Merchant {
  @PrimaryGeneratedColumn('uuid')
  id: string;

  @Column({ unique: true })
  email: string;

  @Exclude()
  @Column()
  passwordHash: string;

  @Column()
  businessName: string;

  @Column({ nullable: true })
  businessType: string;

  @Column({ nullable: true })
  country: string;

  @Column({ type: 'enum', enum: MerchantStatus, default: MerchantStatus.PENDING })
  status: MerchantStatus;

  @Column({ type: 'enum', enum: MerchantRole, default: MerchantRole.MERCHANT })
  role: MerchantRole;

  // Raw API keys are never stored at rest. Only the bcrypt hash is persisted.
  // The plaintext key is returned once upon generation and never stored here.
  @Exclude()
  @Column({ nullable: true })
  apiKeyHash: string;

  /**
   * SHA-256 hex digest of the raw API key, used as a fast indexed lookup key
   * so findMerchantByApiKey does not have to run bcrypt.compare against every
   * merchant. The bcrypt hash (apiKeyHash) is still verified after lookup to
   * provide the actual cryptographic security guarantee.
   *
   * Must be updated atomically alongside apiKeyHash whenever an API key is
   * issued or rotated.
   */
  @Exclude()
  @Index()
  @Column({ nullable: true })
  apiKeyLookupHash: string;

  @Column({ type: 'decimal', precision: 5, scale: 4, default: 0.015 })
  feeRate: number;

  @Column({ default: false })
  sandboxMode: boolean;

  @CreateDateColumn()
  createdAt: Date;

  @UpdateDateColumn()
  updatedAt: Date;

  @DeleteDateColumn()
  deletedAt: Date;
}
