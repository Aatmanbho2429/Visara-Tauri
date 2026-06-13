import { Injectable } from '@angular/core';
import { Observable } from 'rxjs';
import { TauriService } from './tauri.service';
import { BaseResponse } from '../models/base-response.model';
import { LoginData, ValidateTokenData, PeriodicRevalidateData, PlansData, SubscriptionsData } from '../models/auth.model';

export interface LoginPayload { email: string; password: string; }
export interface RegisterPayload {
  first_name: string; last_name: string; email: string; password: string;
  phone_number?: string; company_name?: string; otp_code: string;
}

@Injectable({ providedIn: 'root' })
export class AuthService {
  constructor(private tauri: TauriService) {}

  // Token is stored as a file on PC by Python (~/.pictoria_token)
  // No localStorage involved for token or user data

  login(payload: LoginPayload): Observable<BaseResponse<LoginData>> {
    return this.tauri.invoke<LoginData>('auth_login', {
      email: payload.email,
      password: payload.password
    });
  }

  validateToken(): Observable<BaseResponse<ValidateTokenData>> {
    return this.tauri.invoke<ValidateTokenData>('auth_validate_token');
  }

  /** Silent background re-check of the session/subscription against
   *  Supabase. Runs on a timer (see master.ts) — no loading spinner. */
  periodicRevalidate(): Observable<BaseResponse<PeriodicRevalidateData>> {
    return this.tauri.invokeSilent<PeriodicRevalidateData>('auth_periodic_revalidate');
  }

  /** Instant file-existence check — no Supabase call. Use in route guards. */
  checkSession(): Observable<BaseResponse<null>> {
    return this.tauri.invoke<null>('auth_check_session');
  }

  logout(): Observable<BaseResponse<null>> {
    return this.tauri.invoke<null>('auth_logout');
  }

  getPlans(): Observable<BaseResponse<PlansData>> {
    return this.tauri.invoke<PlansData>('get_plans');
  }

  getUserSubscriptions(): Observable<BaseResponse<SubscriptionsData>> {
    return this.tauri.invoke<SubscriptionsData>('get_user_subscriptions');
  }

  createOrder(userId: string, planId: string): Observable<BaseResponse<any>> {
    return this.tauri.invoke<any>('create_order', { userId, planId });
  }

  verifyPayment(
    razorpayOrderId:   string,
    razorpayPaymentId: string,
    razorpaySignature: string,
    userId:            string,
    planId:            string
  ): Observable<BaseResponse<any>> {
    return this.tauri.invoke<any>('verify_payment', {
      razorpayOrderId,
      razorpayPaymentId,
      razorpaySignature,
      userId,
      planId,
    });
  }

  /** Email a one-time verification code for registration. */
  sendOtp(email: string): Observable<BaseResponse<null>> {
    return this.tauri.invoke<null>('auth_send_otp', { email });
  }

  /** Forgot-password step 1 — email a one-time code to a registered address. */
  forgotPasswordSendOtp(email: string): Observable<BaseResponse<null>> {
    return this.tauri.invoke<null>('auth_forgot_password_send_otp', { email });
  }

  /** Forgot-password step 2 — verify the code; on success a new password is
   *  generated and emailed to the user. */
  forgotPasswordVerifyOtp(email: string, otpCode: string): Observable<BaseResponse<null>> {
    return this.tauri.invoke<null>('auth_forgot_password_verify_otp', { email, otpCode });
  }

  /** Change the logged-in user's password. On success the caller should log
   *  the user out so they re-authenticate with the new password. */
  changePassword(oldPassword: string, newPassword: string): Observable<BaseResponse<null>> {
    return this.tauri.invoke<null>('auth_change_password', { oldPassword, newPassword });
  }

  requestAccess(payload: RegisterPayload): Observable<BaseResponse<null>> {
    return this.tauri.invoke<null>('auth_request_access', {
      firstName:   payload.first_name,
      lastName:    payload.last_name,
      email:       payload.email,
      password:    payload.password,
      phoneNumber: payload.phone_number ?? null,
      companyName: payload.company_name ?? null,
      otpCode:     payload.otp_code,
    });
  }
}
