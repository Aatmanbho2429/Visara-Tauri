import { Injectable } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { firstValueFrom } from 'rxjs';

interface AppConfig {
  apiBaseUrl: string;
  isMaintenance: boolean;
  setTimeMinutes: number;
}

@Injectable({ providedIn: 'root' })
export class ConfigService {
  private config: AppConfig = {
    apiBaseUrl: 'http://127.0.0.1:8765',
    isMaintenance: false,
    setTimeMinutes: 11
  };

  constructor(private http: HttpClient) {}

  async load(): Promise<void> {
    try {
      this.config = await firstValueFrom(
        this.http.get<AppConfig>('./assets/config.json')
      );
    } catch {
      // Falls back to defaults above if config.json is unreachable
    }
  }

  get apiBaseUrl(): string { return this.config.apiBaseUrl; }
  get isMaintenance(): boolean { return this.config.isMaintenance; }
  get setTimeMinutes(): number { return this.config.setTimeMinutes; }
}
