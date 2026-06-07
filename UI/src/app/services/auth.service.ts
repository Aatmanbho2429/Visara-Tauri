import { Injectable } from '@angular/core';
import { Observable } from 'rxjs';
import { TauriService } from './tauri.service';
import { BaseResponse } from '../models/base-response.model';
import { LoginData, ValidateTokenData, PlansData, SubscriptionsData } from '../models/auth.model';

export interface LoginPayload { email: string; password: string; }
export interface RegisterPayload {
  first_name: string; last_name: string; email: string; password: string;
  phone_number?: string; company_name?: string; otp_code: string;
}

@Injectable({ providedIn: 'root' })
export class AuthService {
  constructor(private tauri: TauriService) {}

  // Token is stored as a file on PC by Python (~/.visara_token)
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
