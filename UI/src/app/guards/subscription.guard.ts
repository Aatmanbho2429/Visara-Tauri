import { inject } from '@angular/core';
import { CanActivateFn, Router } from '@angular/router';
import { UserStateService } from '../services/user-state.service';

/**
 * Gates the AI / catalog features behind an active subscription.
 *
 * Runs after `authGuard` (which guarantees a validated user in `userState`).
 * `canSearch` is true only for `trial` / `active`; `expired` / `exhausted`
 * users are redirected to Profile, where they can renew.  This is the UX-level
 * gate — the backend independently unloads the AI model for expired users, so
 * search/indexing also fail there as a second line of defence.
 */
export const subscriptionGuard: CanActivateFn = () => {
  const userState = inject(UserStateService);
  const router = inject(Router);

  if (userState.canSearch) return true;

  sessionStorage.setItem(
    'sub_redirect_msg',
    'Your subscription has ended. Renew to keep using Visara.',
  );
  router.navigate(['/master/profile']);
  return false;
};
