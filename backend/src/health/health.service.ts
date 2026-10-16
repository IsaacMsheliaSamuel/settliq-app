import { Injectable } from '@nestjs/common';
import { ConfigService } from '@nestjs/config';
import { InjectDataSource } from '@nestjs/typeorm';
import { DataSource } from 'typeorm';

export type ComponentStatus = 'healthy' | 'degraded' | 'down';

export interface ComponentHealth {
  status: ComponentStatus;
  latency: number;
}

export interface HealthReport {
  status: ComponentStatus;
  timestamp: string;
  components: {
    database: ComponentHealth;
    stellar: ComponentHealth;
  };
}

@Injectable()
export class HealthService {
  constructor(
    @InjectDataSource()
    private readonly dataSource: DataSource,
    private readonly config: ConfigService,
  ) {}

  async check(): Promise<HealthReport> {
    const [database, stellar] = await Promise.all([
      this.checkDatabase(),
      this.checkStellar(),
    ]);

    const overallStatus = this.aggregate([database.status, stellar.status]);

    return {
      status: overallStatus,
      timestamp: new Date().toISOString(),
      components: { database, stellar },
    };
  }

  // ── Database ──────────────────────────────────────────────────────────────

  async checkDatabase(): Promise<ComponentHealth> {
    const start = Date.now();
    try {
      await this.dataSource.query('SELECT 1');
      return { status: 'healthy', latency: Date.now() - start };
    } catch {
      return { status: 'down', latency: Date.now() - start };
    }
  }

  // ── Stellar Horizon ───────────────────────────────────────────────────────

  async checkStellar(): Promise<ComponentHealth> {
    const horizonUrl =
      this.config.get<string>('STELLAR_HORIZON_URL') ?? 'https://horizon.stellar.org';
    const start = Date.now();
    try {
      const res = await fetch(`${horizonUrl}`, {
        signal: AbortSignal.timeout(5000),
        headers: { Accept: 'application/json' },
      });
      const latency = Date.now() - start;
      return { status: res.ok ? 'healthy' : 'degraded', latency };
    } catch {
      return { status: 'down', latency: Date.now() - start };
    }
  }

  // ── Helpers ───────────────────────────────────────────────────────────────

  private aggregate(statuses: ComponentStatus[]): ComponentStatus {
    if (statuses.some((s) => s === 'down')) return 'down';
    if (statuses.some((s) => s === 'degraded')) return 'degraded';
    return 'healthy';
  }
}
