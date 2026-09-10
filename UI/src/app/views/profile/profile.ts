import { Component, OnInit, ViewChild, inject } from '@angular/core';
import { CommonModule } from '@angular/common';
import {
  ReactiveFormsModule,
  FormGroup,
  FormControl,
  Validators,
  AbstractControl,
  ValidationErrors,
} from '@angular/forms';
import { Router } from '@angular/router';
import { TranslateModule, TranslateService } from '@ngx-translate/core';
import { MessageService } from 'primeng/api';
import { ButtonModule } from 'primeng/button';
import { PasswordModule } from 'primeng/password';
import { BaseComponent } from '../../core/base.component';
import { AuthService } from '../../services/auth/auth.service';
import { AutostartService } from '../../services/autostart/autostart.service';
import { UserStateService } from '../../services/user/user-state.service';
import { SearchStateService } from '../../services/search/search-state.service';
import { PlansDialog } from '../../shared/plans-dialog/plans-dialog';
import { Subscription } from '../../models/response/responseSubscription';

// confirm_password must match new_password.
function passwordMatchValidator(group: AbstractControl): ValidationErrors | null {
  const pw      = group.get('new_password')?.value;
  const confirm = group.get('confirm_password')?.value;
  if (confirm && pw !== confirm) {
    group.get('confirm_password')?.setErrors({ passwordMismatch: true });
    return { passwordMismatch: true };
  }
  if (confirm && pw === confirm) {
    const existing = group.get('confirm_password')?.errors;
    if (existing) {
      const { passwordMismatch, ...rest } = existing;
      group.get('confirm_password')?.setErrors(Object.keys(rest).length ? rest : null);
    }
  }
  return null;
}

@Component({
  selector: 'app-profile',
  imports: [CommonModule, ReactiveFormsModule, TranslateModule, ButtonModule, PasswordModule, PlansDialog],
  templateUrl: './profile.html',
  styleUrl: './profile.scss',
})
export class Profile extends BaseComponent implements OnInit {
  userState = inject(UserStateService);
  private auth        = inject(AuthService);
  private autostart   = inject(AutostartService);
  private messages    = inject(MessageService);
  private router      = inject(Router);
  private searchState = inject(SearchStateService);
  private translate   = inject(TranslateService);

  @ViewChild(PlansDialog) plansDialog!: PlansDialog;

  loading              = true;
  subscriptions: Subscription[] = [];
  historyLoading       = true;

  // Reflects the real OS-level autostart state.
  autostartEnabled  = false;
  autostartBusy     = false;

  // ── Change password ─────────────────────────────────────────────
  showChangePassword     = false;
  changePasswordLoading  = false;
  changePasswordError    = '';
  changePasswordForm = new FormGroup(
    {
      old_password:     new FormControl('', [Validators.required]),
      new_password:     new FormControl('', [Validators.required, Validators.minLength(8)]),
      confirm_password: new FormControl('', [Validators.required]),
    },
    { validators: passwordMatchValidator }
  );

  ngOnInit(): void {
    // If a guard bounced the user here because their subscription ended, explain why.
    const redirectMsg = sessionStorage.getItem('sub_redirect_msg');
    if (redirectMsg) {
      sessionStorage.removeItem('sub_redirect_msg');
      this.messages.add({ key: 'app', severity: 'warn', summary: 'Subscription', detail: redirectMsg, life: 6000 });
    }

    this.handle(this.auth.validateToken(), data => {
      this.loading = false;
      if (data?.user) this.userState.set(data.user);
    }, () => { this.loading = false; });

    this.handle(this.auth.getUserSubscriptions(), data => {
      this.historyLoading = false;
      if (data?.subscriptions) this.subscriptions = data.subscriptions;
    }, () => { this.historyLoading = false; });

    this.refreshAutostartState();
  }

  private refreshAutostartState(): void {
    this.autostart.isEnabled()
      .then(on => this.autostartEnabled = on)
      .catch(err => console.warn('[autostart] isEnabled failed:', err));
  }

  toggleAutostart(): void {
    if (this.autostartBusy) return;
    this.autostartBusy = true;

    const action = this.autostartEnabled ? this.autostart.disable() : this.autostart.enable();
    const willBe = !this.autostartEnabled;

    action
      .then(() => {
        this.autostartEnabled = willBe;
        this.messages.add({
          key: 'app',
          severity: 'success',
          summary:  willBe ? 'Startup launch enabled' : 'Startup launch disabled',
          detail:   willBe
            ? 'Pictoria will start silently in the tray when you sign in.'
            : 'Pictoria will no longer launch automatically.',
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

  // ── Change password ─────────────────────────────────────────────
  toggleChangePassword(): void {
    this.showChangePassword = !this.showChangePassword;
    this.changePasswordError = '';
    this.changePasswordForm.reset();
  }

  submitChangePassword(): void {
    if (this.changePasswordForm.invalid) {
      this.changePasswordForm.markAllAsTouched();
      return;
    }
    this.changePasswordLoading = true;
    this.changePasswordError   = '';
    const { old_password, new_password } = this.changePasswordForm.value;

    this.handle(
      this.auth.changePassword(old_password!, new_password!),
      () => {
        this.changePasswordLoading = false;
        this.messages.add({
          key: 'app',
          severity: 'success',
          summary: this.translate.instant('profile.password.changedTitle'),
          detail:  this.translate.instant('profile.password.changedDetail'),
          life: 4000,
        });
        // Force re-authentication with the new credentials.
        setTimeout(() => this.logout(), 1500);
      },
      err => {
        this.changePasswordLoading = false;
        this.changePasswordError   = err.message;
      },
    );
  }

  // Tear down the session and return to the login screen.
  private logout(): void {
    this.auth.logout().subscribe();
    this.userState.clear();
    this.searchState.reset();
    this.router.navigate(['/']);
  }

  isInvalid(field: string): boolean {
    const ctrl = this.changePasswordForm.get(field);
    return !!(ctrl?.invalid && ctrl.touched);
  }

  hasError(field: string, error: string): boolean {
    return !!this.changePasswordForm.get(field)?.hasError(error);
  }

  get statusLabel(): string {
    const map: Record<string, string> = {
      trial: 'Trial', active: 'Active', expired: 'Expired', exhausted: 'Limit Reached',
    };
    return map[this.userState.subscriptionStatus] ?? 'Unknown';
  }

  get subscriptionEndFormatted(): string {
    const end = this.userState.user?.subscriptionEnd;
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
