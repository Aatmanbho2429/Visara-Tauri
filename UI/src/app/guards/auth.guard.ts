import { inject } from '@angular/core';
import { CanActivateFn, Router } from '@angular/router';
import { firstValueFrom } from 'rxjs';
import { AuthService } from '../services/auth.service';
import { UserStateService } from '../services/user-state.service';

export const authGuard: CanActivateFn = async () => {
  const auth      = inject(AuthService);
  const userState = inject(UserStateService);
  const router    = inject(Router);

  // If userState is already populated (e.g. we just came from a successful
  // login), skip the Supabase round-trip — the token is valid by definition.
  if (userState.user) return true;

  // Cold start: app opened with an existing token file but no in-memory user.
  // Validate against Supabase once to re-hydrate user state and load the model.
  const res = await firstValueFrom(auth.validateToken());

  if (res.success && res.data?.user) {
    userState.set(res.data.user);
    return true;
  }

  if (res.message) sessionStorage.setItem('auth_redirect_msg', res.message);
  router.navigate(['/']);
  return false;
};
