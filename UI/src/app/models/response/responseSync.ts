// Library-sync event payloads (the `library_sync_*` event stream) — mirrors
// UI/src-tauri/src/models/response/response_sync.rs.

export interface FileError {
  file: string;
  reason: string;
}

export interface FileTypeCount {
  done: number;
  total: number;
}

// Mirrors core::progress::ProgressSnapshot (UI/src-tauri/src/core/progress.rs) —
// shared by `library_sync_progress` and the sidecar-load "preparing" snapshot.
export interface ProgressSnapshot {
  active: boolean;
  phase: string;
  done: number;
  total: number;
  current: string;
  percent: number;
  etaSec: number;
  errors: number;
  elapsed: number;
  fileTypes: Record<string, FileTypeCount>;
}

export interface responseLibrarySyncStarted {
  path: string;
}

export interface responseLibrarySyncProgress {
  path: string;
  progress: ProgressSnapshot;
}

export interface responseLibrarySyncComplete {
  path: string;
  errors: number;
  failed: FileError[];
  imageCount: number;
}

export interface responseLibrarySyncError {
  path: string;
  message: string;
}
