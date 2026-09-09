// Single source of truth for every Tauri *broadcast* event name — see
// .claude/rules/tauri-ipc.md. `<command>_response` events aren't listed here:
// ZoneWrapperService.invoke() derives that name from TAURI_COMMANDS itself,
// so there's nothing else that needs to know it.
export const TAURI_EVENTS = {
  SEARCH_PROGRESS: 'search_progress',
  SEARCH_COMPLETE: 'search_complete',
  SEARCH_ERROR: 'search_error',
  LIBRARY_SYNC_STARTED: 'library_sync_started',
  LIBRARY_SYNC_PROGRESS: 'library_sync_progress',
  LIBRARY_SYNC_COMPLETE: 'library_sync_complete',
  LIBRARY_SYNC_ERROR: 'library_sync_error',
  UPDATE_AVAILABLE: 'update_available',
  UPDATE_PROGRESS: 'update_progress',
  UPDATE_ERROR: 'update_error',
  UPDATE_NOT_AVAILABLE: 'update_not_available',
  HOTKEY_PRESSED: 'hotkey_pressed',
  SIDECAR_CRASHED: 'sidecar_crashed',
  TAGS_UPDATED: 'tags_updated',
} as const;
