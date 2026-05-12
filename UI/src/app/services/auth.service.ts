import { Injectable } from '@angular/core';
import { Observable } from 'rxjs';
import { tap } from 'rxjs/operators';
import { ApiService } from './api.service';
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
  constructor(private api: ApiService) {}

  login(payload: LoginPayload): Observable<BaseResponse<LoginData>> {
    return this.api.post<LoginData>('/auth/login', payload).pipe(
      tap(res => { if (res.success && res.data?.token) this.saveToken(res.data.token); })
    );
  }

  validateToken(): Observable<BaseResponse<ValidateTokenData>> {
    return this.api.get<ValidateTokenData>('/auth/validate-token', this.getToken() ?? undefined);
  }

  requestAccess(payload: RegisterPayload): Observable<BaseResponse<null>> {
    return this.api.post<null>('/auth/request-access', payload);
  }

  logout(): Observable<BaseResponse<null>> {
    return this.api.post<null>('/auth/logout', {}, this.getToken() ?? undefined).pipe(
      tap(() => this.clearToken())
    );
  }

  getToken(): string | null { return localStorage.getItem(TOKEN_KEY); }
  saveToken(token: string): void { localStorage.setItem(TOKEN_KEY, token); }
  clearToken(): void { localStorage.removeItem(TOKEN_KEY); }
  isLoggedIn(): boolean { return !!this.getToken(); }
}
