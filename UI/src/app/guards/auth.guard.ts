import { inject } from '@angular/core';
import { CanActivateFn, Router } from '@angular/router';
import { firstValueFrom } from 'rxjs';
import { AuthService } from '../services/auth.service';

export const authGuard: CanActivateFn = async () => {
  const auth   = inject(AuthService);
  const router = inject(Router);

  const res = await firstValueFrom(auth.validateToken());

  if (res.success) return true;

  if (res.message) sessionStorage.setItem('auth_redirect_msg', res.message);
  router.navigate(['/']);
  return false;
};
