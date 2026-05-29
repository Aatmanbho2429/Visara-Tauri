import { Component, OnInit, ViewChild, inject } from '@angular/core';
import { CommonModule } from '@angular/common';
import { TranslateModule } from '@ngx-translate/core';
import { MessageService } from 'primeng/api';
import { BaseComponent } from '../../core/base.component';
import { AuthService } from '../../services/auth.service';
import { TauriService } from '../../services/tauri.service';
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
  private auth     = inject(AuthService);
  private tauri    = inject(TauriService);
  private messages = inject(MessageService);

  @ViewChild(PlansDialog) plansDialog!: PlansDialog;

  loading              = true;
  subscriptions: Subscription[] = [];
  historyLoading       = true;

  /** Reflects the real OS-level autostart state. */
  autostartEnabled  = false;
  autostartBusy     = false;

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

    this.refreshAutostartState();
  }

  private refreshAutostartState(): void {
    this.tauri.autostartIsEnabled()
      .then(on => this.autostartEnabled = on)
      .catch(err => console.warn('[autostart] isEnabled failed:', err));
  }

  toggleAutostart(): void {
    if (this.autostartBusy) return;
    this.autostartBusy = true;

    const action = this.autostartEnabled ? this.tauri.autostartDisable() : this.tauri.autostartEnable();
    const willBe = !this.autostartEnabled;

    action
      .then(() => {
        this.autostartEnabled = willBe;
        this.messages.add({
          key: 'app',
          severity: 'success',
          summary:  willBe ? 'Startup launch enabled' : 'Startup launch disabled',
          detail:   willBe
            ? 'Visara will start silently in the tray when you sign in.'
            : 'Visara will no longer launch automatically.',
          life:     4000,
        });
      })
      .catch(err => {
        console.error('[autostart] toggle failed:', err);
        this.messages.add({
          key: 'app',
          severity: 'error',
          summary:  'Could not change startup setting',
          detail:   String(err),
          life:     5000,
        });
        // Re-sync UI with whatever the OS actually reports.
        this.refreshAutostartState();
      })
      .finally(() => this.autostartBusy = false);
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
