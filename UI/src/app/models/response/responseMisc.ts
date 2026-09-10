// One-off event payloads — mirrors UI/src-tauri/src/models/response/response_misc.rs
// and response_update.rs.

export interface responseHotkeyPressed {
  hasImage: boolean;
  imagePath: string;
}

// Emitted by core::sidecar's watchdog when the sidecar process exits on its
// own (not via shutdown()) — a crash, not a normal shutdown. `recovering:
// true` means it's being relaunched; `false` means the watchdog gave up
// after repeated failures and a human needs to restart the app.
export interface responseSidecarCrashed {
  recovering: boolean;
  message: string;
}

export interface responseUpdateAvailable {
  version: string;
  notes: string;
}

export interface responseUpdateProgress {
  downloaded: number;
  total: number | null;
}
