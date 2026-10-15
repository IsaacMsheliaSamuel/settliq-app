import { Injectable, CanActivate, ExecutionContext, ForbiddenException } from '@nestjs/common';
import { Reflector } from '@nestjs/core';
import { ROLES_KEY } from '../decorators/roles.decorator';
import { MerchantRole } from '../../merchants/entities/merchant.entity';

/**
 * Enforces role-based access by reading the required roles attached via the
 * @Roles() decorator and comparing them against the authenticated user's role.
 *
 * Must be applied AFTER JwtAuthGuard (or any guard that populates req.user).
 * Requests whose user role does not match any of the required roles receive a
 * 403 Forbidden response.
 *
 * MerchantRole.SUPERADMIN is always granted access when the route requires
 * MerchantRole.ADMIN, matching the platform's privilege hierarchy.
 */
@Injectable()
export class RolesGuard implements CanActivate {
  constructor(private readonly reflector: Reflector) {}

  canActivate(context: ExecutionContext): boolean {
    const requiredRoles = this.reflector.getAllAndOverride<MerchantRole[]>(
      ROLES_KEY,
      [context.getHandler(), context.getClass()],
    );

    // If no @Roles() decorator is present the route is open to all authenticated users.
    if (!requiredRoles || requiredRoles.length === 0) {
      return true;
    }

    const request = context.switchToHttp().getRequest();
    const user = request.user as { role?: MerchantRole } | undefined;

    if (!user?.role) {
      throw new ForbiddenException('Access denied: no role assigned');
    }

    // SUPERADMIN always satisfies an ADMIN requirement.
    if (user.role === MerchantRole.SUPERADMIN) {
      return true;
    }

    if (requiredRoles.includes(user.role)) {
      return true;
    }

    throw new ForbiddenException('Access denied: insufficient role');
  }
}
