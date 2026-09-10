import { Injectable } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { firstValueFrom } from 'rxjs';

// `apiBaseUrl` was dropped from this shape — it belonged to the retired
// HTTP-proxy ApiService (Rust talks to Supabase directly now); config.json
// on disk may still carry the key, it's just ignored.
interface AppConfig {
  isMaintenance: boolean;
  setTimeMinutes: number;
}

@Injectable({ providedIn: 'root' })
export class ConfigService {
  private config: AppConfig = {
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

  get isMaintenance(): boolean { return this.config.isMaintenance; }
  get setTimeMinutes(): number { return this.config.setTimeMinutes; }
}
