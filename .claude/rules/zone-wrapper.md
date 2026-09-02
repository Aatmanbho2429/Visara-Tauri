---
paths:
  - "src/app/services/**/*.ts"
  - "src/app/core/zone-wrapper/**/*.ts"
---

# NgZone wrapper

`invoke()` and `listen()` from `@tauri-apps/api` resolve outside Angular's zone, so the UI won't update on their result unless it's pushed back in.

- `ZoneWrapperService` (`core/zone-wrapper/zone-wrapper.service.ts`) is the only file that imports `@tauri-apps/api`.
- Every service method that talks to Tauri calls `zoneWrapper.invoke()` or `zoneWrapper.listen()` — never `invoke`/`listen` directly.
- `zoneWrapper.invoke()` unwraps the `ApiResponse` envelope and returns the plain payload, already run inside `NgZone`, so callers never see the envelope or have to think about zones.