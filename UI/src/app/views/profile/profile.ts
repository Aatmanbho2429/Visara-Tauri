import { Component, OnInit, ViewChild, inject } from '@angular/core';
import { CommonModule } from '@angular/common';
import { TranslateModule } from '@ngx-translate/core';
import { BaseComponent } from '../../core/base.component';
import { AuthService } from '../../services/auth.service';
import { UserStateService } from '../../services/user-state.service';
import { PlansDialog } from '../../shared/plans-dialog/plans-dialog';
import { Subscription } from '../../models/auth.model';

@Component({
  selector: 'app-profile',
  imports: [CommonModule, TranslateModule, PlansDialog],
  templateUrl: './profile.html',
  styleUrl: './profile.scss',
})
export class Profile extends BaseComponent implements OnInit {
  userState = inject(UserStateService);
  private auth = inject(AuthService);

  @ViewChild(PlansDialog) plansDialog!: PlansDialog;

  loading              = true;
  subscriptions: Subscription[] = [];
  historyLoading       = true;

  ngOnInit(): void {
    this.handle(this.auth.validateToken(), res => {
      this.loading = false;
      if (res.success && res.data?.user) this.userState.set(res.data.user);
    });

    this.handle(this.auth.getUserSubscriptions(), res => {
      this.historyLoading = false;
      if (res.success && res.data?.subscriptions) {
        this.subscriptions = res.data.subscriptions;
      }
    });
  }

  openPlans(): void { this.plansDialog.open(); }

  get statusLabel(): string {
    const map: Record<string, string> = {
      trial: 'Trial', active: 'Active', expired: 'Expired', exhausted: 'Limit Reached',
    };
    return map[this.userState.subscriptionStatus] ?? 'Unknown';
  }

  get subscriptionEndFormatted(): string {
    const end = this.userState.user?.subscription_end;
    if (!end) return '';
    return new Date(end).toLocaleDateString('en-IN', { day: 'numeric', month: 'long', year: 'numeric' });
  }

  formatDate(date: string): string {
    return new Date(date).toLocaleDateString('en-IN', { day: 'numeric', month: 'short', year: 'numeric' });
  }

  formatAmount(amount: string, currency: string): string {
    const symbol = currency === 'INR' ? '₹' : currency;
    return `${symbol}${Number(amount).toLocaleString('en-IN')}`;
  }
}
