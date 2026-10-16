import {
  Controller,
  Get,
  HttpCode,
  HttpStatus,
  Res,
} from '@nestjs/common';
import { ApiTags, ApiOperation, ApiOkResponse, ApiServiceUnavailableResponse } from '@nestjs/swagger';
import { Response } from 'express';
import { HealthService, HealthReport } from './health.service';

@ApiTags('health')
@Controller('health')
export class HealthController {
  constructor(private readonly healthService: HealthService) {}

  /**
   * GET /api/v1/health/admin
   *
   * Returns a JSON health report consumed by monitoring/statuspage/worker.js.
   * Responds with HTTP 200 when all critical components are healthy, and
   * HTTP 503 when at least one critical component is down or degraded.
   */
  @Get('admin')
  @ApiOperation({ summary: 'Admin health check — database, Stellar Horizon, and system status' })
  @ApiOkResponse({ description: 'All components healthy' })
  @ApiServiceUnavailableResponse({ description: 'One or more components are degraded or down' })
  async adminHealth(@Res() res: Response): Promise<void> {
    const report: HealthReport = await this.healthService.check();

    const httpStatus =
      report.status === 'healthy' ? HttpStatus.OK : HttpStatus.SERVICE_UNAVAILABLE;

    res.status(httpStatus).json(report);
  }
}
