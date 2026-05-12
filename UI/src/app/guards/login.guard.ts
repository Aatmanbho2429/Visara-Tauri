import { inject } from '@angular/core';
import { CanActivateFn, Router } from '@angular/router';
import { firstValueFrom } from 'rxjs';
import { AuthService } from '../services/auth.service';

export const loginGuard: CanActivateFn = async () => {
  const auth   = inject(AuthService);
  const router = inject(Router);

  const res = await firstValueFrom(auth.validateToken());

  if (res.success) {
    router.navigate(['/master']);
    return false;
  }

  return true;
};
