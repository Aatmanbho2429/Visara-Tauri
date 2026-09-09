// Library / watched-folder request payloads — mirrors
// UI/src-tauri/src/models/request/request_library.rs.

export interface requestRemoveFolder {
  path: string;
  purge: boolean;
}

export interface requestSetPaused {
  path: string;
  paused: boolean;
}
