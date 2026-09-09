import { Injectable } from '@angular/core';
import { disable, enable, isEnabled } from '@tauri-apps/plugin-autostart';

// Launch-at-login toggle (shown on the Profile screen). This wraps the
// `@tauri-apps/plugin-autostart` plugin API directly rather than a Rust
// `#[tauri::command]` — there's no Rust-side logic behind it, so there's
// nothing for `ZoneWrapperService` to correlate.
@Injectable({ providedIn: 'root' })
export class AutostartService {
  isEnabled(): Promise<boolean> { return isEnabled(); }
  enable():    Promise<void>    { return enable(); }
  disable():   Promise<void>    { return disable(); }
}
