import { Injectable } from '@angular/core';
import { HttpClient, HttpHeaders } from '@angular/common/http';
import { Observable } from 'rxjs';
import { BaseResponse } from '../models/base-response.model';
import { ConfigService } from './config.service';

@Injectable({ providedIn: 'root' })
export class ApiService {
  constructor(
    private http: HttpClient,
    private config: ConfigService
  ) {}

  get<T>(endpoint: string, token?: string): Observable<BaseResponse<T>> {
    return this.http.get<BaseResponse<T>>(
      `${this.config.apiBaseUrl}${endpoint}`,
      { headers: this.buildHeaders(token) }
    );
  }

  post<T>(endpoint: string, body: unknown, token?: string): Observable<BaseResponse<T>> {
    return this.http.post<BaseResponse<T>>(
      `${this.config.apiBaseUrl}${endpoint}`,
      body,
      { headers: this.buildHeaders(token) }
    );
  }

  private buildHeaders(token?: string): HttpHeaders {
    let headers = new HttpHeaders({ 'Content-Type': 'application/json' });
    if (token) headers = headers.set('Authorization', `Bearer ${token}`);
    return headers;
  }
}
