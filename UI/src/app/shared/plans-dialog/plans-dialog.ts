import { Component, NgZone, inject } from '@angular/core';
import { CommonModule } from '@angular/common';
import { DialogModule } from 'primeng/dialog';
import { BaseComponent } from '../../core/base.component';
import { AuthService } from '../../services/auth/auth.service';
import { UserStateService } from '../../services/user/user-state.service';
import { Plan } from '../../models/response/responseSubscription';

@Component({
  selector: 'app-plans-dialog',
  imports: [CommonModule, DialogModule],
  templateUrl: './plans-dialog.html',
})
export class PlansDialog extends BaseComponent {

  private auth      = inject(AuthService);
  private zone      = inject(NgZone);
  userState         = inject(UserStateService);

  visible      = false;
  loading      = false;
  plans: Plan[] = [];

  payingPlanId: string | null = null;
  payError     = '';
  paySuccess   = false;
  successMsg   = '';

  open(): void {
    this.visible     = true;
    this.loading     = true;
    this.plans       = [];
    this.payError    = '';
    this.paySuccess  = false;
    this.payingPlanId = null;

    this.handle(
      this.auth.getPlans(),
      data => { this.loading = false; this.plans = data.plans; this.cdr.detectChanges(); },
      () => { this.loading = false; this.cdr.detectChanges(); },
    );
  }

  close(): void {
    this.visible      = false;
    this.payingPlanId = null;
    this.payError     = '';
    this.paySuccess   = false;
    this.cdr.detectChanges();
  }

  // ── Payment ───────────────────────────────────────────────────
  buyPlan(plan: Plan): void {
    if (!this.userState.userId || this.payingPlanId) return;

    this.payingPlanId = plan.id;
    this.payError     = '';

    this.handle(
      this.auth.createOrder(this.userState.userId!, plan.id),
      d => {
        this.openRazorpay({
          key:         d.keyId,
          amount:      d.amount,
          currency:    d.currency,
          name:        'Pictoria',
          description: `${plan.name} Plan`,
          order_id:    d.orderId,
          prefill: {
            name:    d.user?.name    ?? '',
            email:   d.user?.email   ?? '',
            contact: d.user?.phone   ?? '',
          },
          theme: { color: '#d946ef' },
          handler: (response: any) => {
            this.zone.run(() => {
              this.handle(
                this.auth.verifyPayment(
                  response.razorpay_order_id,
                  response.razorpay_payment_id,
                  response.razorpay_signature,
                  this.userState.userId!,
                  plan.id,
                ),
                verData => {
                  this.payingPlanId = null;
                  this.userState.updateSubscription({
                    subscriptionStatus: verData.subscriptionStatus,
                    subscriptionEnd:    verData.subscriptionEnd,
                    daysRemaining:      verData.daysRemaining,
                  });
                  this.paySuccess = true;
                  this.successMsg = `${plan.name} plan activated! ${verData.daysRemaining} days remaining.`;
                  setTimeout(() => this.close(), 2800);
                  this.cdr.detectChanges();
                },
                err => {
                  this.payingPlanId = null;
                  this.payError = err.message || 'Payment verification failed.';
                  this.cdr.detectChanges();
                },
              );
            });
          },
        });
      },
      err => {
        this.payError     = err.message || 'Failed to create order. Please try again.';
        this.payingPlanId = null;
        this.cdr.detectChanges();
      },
    );
  }

  private openRazorpay(options: any): void {
    const existing = document.getElementById('rzp-script');
    if (existing) {
      this.launchRazorpay(options);
      return;
    }
    const script    = document.createElement('script');
    script.id       = 'rzp-script';
    script.src      = 'https://checkout.razorpay.com/v1/checkout.js';
    script.onload   = () => this.launchRazorpay(options);
    script.onerror  = () => {
      this.zone.run(() => {
        this.payError     = 'Could not load payment gateway. Check your internet connection.';
        this.payingPlanId = null;
        this.cdr.detectChanges();
      });
    };
    document.body.appendChild(script);
  }

  private launchRazorpay(options: any): void {
    const rzp = new (window as any).Razorpay({
      ...options,
      modal: {
        ondismiss: () => {
          this.zone.run(() => {
            this.payingPlanId = null;
            this.cdr.detectChanges();
          });
        }
      }
    });
    rzp.on('payment.failed', () => {
      this.zone.run(() => {
        this.payError     = 'Payment failed. Please try again.';
        this.payingPlanId = null;
        this.cdr.detectChanges();
      });
    });
    rzp.open();
  }

  // ── Helpers ───────────────────────────────────────────────────
  formatAmount(plan: Plan): string {
    const symbol = plan.currency === 'INR' ? '₹' : plan.currency;
    return `${symbol}${Number(plan.amount).toLocaleString('en-IN')}`;
  }

  durationLabel(plan: Plan): string {
    if (plan.duration === 30)   return '1 Month';
    if (plan.duration === 90)  return '3 Months';
    if (plan.duration === 365) return '1 Year';
    return `${plan.duration} Days`;
  }

  isRecommended(plan: Plan): boolean { return plan.duration === 90; }

  planTagline(plan: Plan): string {
    if (plan.duration <= 30)  return 'No commitment. Full AI power for 30 days — perfect for a quick project.';
    if (plan.duration <= 90) return `The professional's choice. Search as much as you want, every single day.`;
    return 'Go all in. A full year of unlimited access and you save 33% vs monthly.';
  }

  planIcon(plan: Plan): string {
    if (plan.duration <= 30)  return 'pi-bolt';
    if (plan.duration <= 90) return 'pi-star';
    return 'pi-crown';
  }
}
