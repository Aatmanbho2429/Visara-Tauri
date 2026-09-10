import { Injectable } from '@angular/core';
import { BehaviorSubject } from 'rxjs';
import { User } from '../../models/response/responseAuth';

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
    return `${u.firstName ?? ''} ${u.lastName ?? ''}`.trim();
  }

  get initials(): string {
    const u = this.user;
    if (!u) return '?';
    return `${u.firstName?.[0] ?? ''}${u.lastName?.[0] ?? ''}`.toUpperCase() || '?';
  }

  get userId(): string | null { return this.user?.id ?? null; }

  updateSubscription(data: { subscriptionStatus: string; subscriptionEnd: string; daysRemaining: number }): void {
    if (!this.user) return;
    this.set({ ...this.user, ...data });
  }

  get subscriptionStatus(): string {
    return (this.user?.subscriptionStatus ?? 'trial').toLowerCase();
  }

  get canSearch(): boolean {
    const s = this.subscriptionStatus;
    return s === 'trial' || s === 'active';
  }

  get daysRemaining(): number | null {
    const u = this.user;
    if (!u) return null;
    if (u.daysRemaining != null) return u.daysRemaining;
    if (u.subscriptionEnd) {
      const diff = new Date(u.subscriptionEnd).getTime() - Date.now();
      return Math.max(0, Math.ceil(diff / (1000 * 60 * 60 * 24)));
    }
    return null;
  }
}
