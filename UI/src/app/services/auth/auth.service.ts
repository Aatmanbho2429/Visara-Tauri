import { Injectable } from '@angular/core';
import { Observable } from 'rxjs';
import { TAURI_COMMANDS } from '../../core/tauri/tauri-commands.const';
import { ZoneWrapperService } from '../../core/zone-wrapper/zone-wrapper.service';
import { requestChangePassword, requestEmail, requestLogin, requestRequestAccess, requestVerifyOtp } from '../../models/request/requestAuth';
import { responseLogin, responsePeriodicRevalidate, responseValidateToken } from '../../models/response/responseAuth';
import { responseCreateOrder, responsePlans, responseSubscriptions, responseVerifyPayment } from '../../models/response/responseSubscription';

@Injectable({ providedIn: 'root' })
export class AuthService {
  constructor(private zoneWrapper: ZoneWrapperService) {}

  // Token is stored as a file/keychain entry on the OS. No localStorage
  // involved for token or user data.

  login(payload: requestLogin): Observable<responseLogin> {
    return this.zoneWrapper.invoke<responseLogin>(TAURI_COMMANDS.AUTH_LOGIN, payload);
  }

  validateToken(): Observable<responseValidateToken> {
    return this.zoneWrapper.invoke<responseValidateToken>(TAURI_COMMANDS.AUTH_VALIDATE_TOKEN);
  }

  // Silent background re-check of the session/subscription against
  // Supabase. Runs on a timer (see master.ts) — no loading spinner.
  periodicRevalidate(): Observable<responsePeriodicRevalidate> {
    return this.zoneWrapper.invokeSilent<responsePeriodicRevalidate>(TAURI_COMMANDS.AUTH_PERIODIC_REVALIDATE);
  }

  // Instant file-existence check — no Supabase call. Use in route guards.
  checkSession(): Observable<null> {
    return this.zoneWrapper.invoke<null>(TAURI_COMMANDS.AUTH_CHECK_SESSION);
  }

  logout(): Observable<null> {
    return this.zoneWrapper.invoke<null>(TAURI_COMMANDS.AUTH_LOGOUT);
  }

  sendOtp(email: string): Observable<null> {
    return this.zoneWrapper.invoke<null>(TAURI_COMMANDS.AUTH_SEND_OTP, { email } as requestEmail);
  }

  forgotPasswordSendOtp(email: string): Observable<null> {
    return this.zoneWrapper.invoke<null>(TAURI_COMMANDS.AUTH_FORGOT_PASSWORD_SEND_OTP, { email } as requestEmail);
  }

  forgotPasswordVerifyOtp(email: string, otpCode: string): Observable<null> {
    return this.zoneWrapper.invoke<null>(TAURI_COMMANDS.AUTH_FORGOT_PASSWORD_VERIFY_OTP, { email, otpCode } as requestVerifyOtp);
  }

  // Change the logged-in user's password. On success the caller should log
  // the user out so they re-authenticate with the new password.
  changePassword(oldPassword: string, newPassword: string): Observable<null> {
    return this.zoneWrapper.invoke<null>(TAURI_COMMANDS.AUTH_CHANGE_PASSWORD, { oldPassword, newPassword } as requestChangePassword);
  }

  requestAccess(payload: requestRequestAccess): Observable<null> {
    return this.zoneWrapper.invoke<null>(TAURI_COMMANDS.AUTH_REQUEST_ACCESS, payload);
  }

  // ── Subscription / payments ──────────────────────────────────────
  // Kept on AuthService rather than split into a SubscriptionService: the
  // three call sites (plans-dialog, profile) already inject AuthService for
  // session state, and none of these need their own entity beyond that.

  getPlans(): Observable<responsePlans> {
    return this.zoneWrapper.invoke(TAURI_COMMANDS.SUBSCRIPTION_GET_PLANS);
  }

  getUserSubscriptions(): Observable<responseSubscriptions> {
    return this.zoneWrapper.invoke(TAURI_COMMANDS.SUBSCRIPTION_GET_USER_SUBSCRIPTIONS);
  }

  createOrder(userId: string, planId: string): Observable<responseCreateOrder> {
    return this.zoneWrapper.invoke(TAURI_COMMANDS.SUBSCRIPTION_CREATE_ORDER, { userId, planId });
  }

  verifyPayment(
    razorpayOrderId:   string,
    razorpayPaymentId: string,
    razorpaySignature: string,
    userId:            string,
    planId:            string
  ): Observable<responseVerifyPayment> {
    return this.zoneWrapper.invoke(TAURI_COMMANDS.SUBSCRIPTION_VERIFY_PAYMENT, {
      razorpayOrderId,
      razorpayPaymentId,
      razorpaySignature,
      userId,
      planId,
    });
  }
}
