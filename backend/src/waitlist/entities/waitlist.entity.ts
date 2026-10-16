import { Entity, PrimaryGeneratedColumn, Column, CreateDateColumn } from 'typeorm';

@Entity('waitlist')
export class WaitlistEntry {
  @PrimaryGeneratedColumn('uuid')
  id: string;

  @Column({ unique: true })
  email: string;

  @Column({ unique: true, nullable: true })
  username: string;

  @Column({ nullable: true })
  businessName: string;

  @Column({ nullable: true })
  country: string;

  @Column({ default: 0 })
  referrals: number;

  @Column({ nullable: true })
  referralCode: string;

  @Column({ default: false })
  unsubscribed: boolean;

  /** SHA-256 hash of the unsubscribe token; raw token is sent in email and never persisted */
  @Column({ nullable: true })
  unsubscribeTokenHash: string;

  @Column({ default: false })
  notified: boolean;

  @CreateDateColumn()
  createdAt: Date;
}
