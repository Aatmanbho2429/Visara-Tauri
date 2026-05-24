import { inject } from '@angular/core';
import { CanActivateFn, Router } from '@angular/router';
import { firstValueFrom } from 'rxjs';
import { AuthService } from '../services/auth.service';

export const loginGuard: CanActivateFn = async () => {
  const auth   = inject(AuthService);
  const router = inject(Router);

  // Instant check — just sees if the token file exists, no Supabase call.
  // If a token is on disk the user is already logged in; send them to master.
  const res = await firstValueFrom(auth.checkSession());

  if (res.success) {
    router.navigate(['/master']);
    return false;
  }

  return true;
};
