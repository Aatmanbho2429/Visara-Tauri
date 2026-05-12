import { Injectable } from '@angular/core';
import { Observable } from 'rxjs';
import { tap } from 'rxjs/operators';
import { TauriService } from './tauri.service';
import { BaseResponse } from '../models/base-response.model';
import { LoginData, ValidateTokenData } from '../models/auth.model';

const TOKEN_KEY = 'visara_token';

export interface LoginPayload { email: string; password: string; }
export interface RegisterPayload {
  first_name: string; last_name: string; email: string; password: string;
  phone_number?: string; company_name?: string;
}

@Injectable({ providedIn: 'root' })
export class AuthService {
  constructor(private tauri: TauriService) {}

  login(payload: LoginPayload): Observable<BaseResponse<LoginData>> {
    return this.tauri.invoke<LoginData>('auth_login', {
      email: payload.email,
      password: payload.password
    }).pipe(
      tap(res => { if (res.success && res.data?.token) this.saveToken(res.data.token); })
    );
  }

  validateToken(): Observable<BaseResponse<ValidateTokenData>> {
    return this.tauri.invoke<ValidateTokenData>('auth_validate_token');
  }

  requestAccess(payload: RegisterPayload): Observable<BaseResponse<null>> {
    return this.tauri.invoke<null>('auth_request_access', {
      firstName:   payload.first_name,
      lastName:    payload.last_name,
      email:       payload.email,
      password:    payload.password,
      phoneNumber: payload.phone_number ?? null,
      companyName: payload.company_name ?? null,
    });
  }

  getToken(): string | null { return localStorage.getItem(TOKEN_KEY); }
  saveToken(token: string): void { localStorage.setItem(TOKEN_KEY, token); }
  clearToken(): void { localStorage.removeItem(TOKEN_KEY); }
  isLoggedIn(): boolean { return !!this.getToken(); }
}
