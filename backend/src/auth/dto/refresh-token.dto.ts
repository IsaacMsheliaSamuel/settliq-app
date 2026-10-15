import { IsString, IsNotEmpty } from 'class-validator';
import { ApiProperty } from '@nestjs/swagger';

export class RefreshTokenDto {
  @ApiProperty({ description: 'Opaque refresh token issued at login or registration' })
  @IsString()
  @IsNotEmpty()
  refreshToken: string;
}
