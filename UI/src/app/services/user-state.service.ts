import { Injectable } from '@angular/core';
import { BehaviorSubject } from 'rxjs';
import { User } from '../models/auth.model';

@Injectable({ providedIn: 'root' })
export class UserStateService {
  private subject = new BehaviorSubject<User | null>(null);
  user$ = this.subject.asObservable();

  get user(): User | null { return this.subject.value; }

  set(user: User): void { this.subject.next(user); }
  clear(): void { this.subject.next(null); }

  get fullName(): string {
    const u = this.user;
    if (!u) return '';
    return `${u.first_name ?? ''} ${u.last_name ?? ''}`.trim();
  }

  get initials(): string {
    const u = this.user;
    if (!u) return '?';
    return `${u.first_name?.[0] ?? ''}${u.last_name?.[0] ?? ''}`.toUpperCase() || '?';
  }

  get subscriptionStatus(): string {
    return this.user?.subscription_status ?? 'trial';
  }

  get canSearch(): boolean {
    const s = this.subscriptionStatus;
    return s === 'trial' || s === 'active';
  }

  get daysRemaining(): number | null {
    const u = this.user;
    if (!u) return null;
    if (u.days_remaining != null) return u.days_remaining;
    if (u.subscription_end) {
      const diff = new Date(u.subscription_end).getTime() - Date.now();
      return Math.max(0, Math.ceil(diff / (1000 * 60 * 60 * 24)));
    }
    return null;
  }
}
