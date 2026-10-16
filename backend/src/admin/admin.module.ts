import { Module } from '@nestjs/common';
import { AdminWaitlistController } from './admin-waitlist.controller';
import { WaitlistModule } from '../waitlist/waitlist.module';
import { RolesGuard } from '../auth/guards/roles.guard';

@Module({
  imports: [WaitlistModule],
  controllers: [AdminWaitlistController],
  providers: [RolesGuard],
})
export class AdminModule {}
