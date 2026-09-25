---
paths:
  - "UI/src/app/services/**/*.ts"
  - "UI/src/app/core/zone-wrapper/**/*.ts"
---

# NgZone wrapper

`invoke()` and `listen()` from `@tauri-apps/api` resolve outside Angular's zone, so the UI won't update on their result unless it's pushed back in.

- `ZoneWrapperService` (`core/zone-wrapper/zone-wrapper.service.ts`) is the only file that imports `@tauri-apps/api`.
- Every service method that talks to Tauri calls `zoneWrapper.invoke()` or `zoneWrapper.listen()` — never `invoke`/`listen` directly.
- `zoneWrapper.invoke()` unwraps the `ApiResponse` envelope and returns the plain payload, already run inside `NgZone`, so callers never see the envelope or have to think about zones.
- Services pass command and event names from the registries in `core/tauri/`
  (`tauri-commands.const.ts`, `tauri-events.const.ts`), never string literals.

## Five call shapes

| Method | Use |
|---|---|
| `invoke<T>()` | Normal command. Shows the global loader; resolves with `data`, errors with `ApiError` on a non-2xx `statusCode` |
| `invokeSilent<T>()` | Background or periodic work (e.g. licence re-validation); no loader |
| `invokeFireAndForget()` | A command with no `_response` counterpart — `search_start`, `update_check`, `update_install` report entirely through their own broadcast events |
| `listen<T>()` | Subscribe to a broadcast event. Returns the full `ApiResponse<T>` envelope, since a stream can carry more than one outcome over its lifetime |
| `toAssetUrl()` | `convertFileSrc` passthrough, so this stays the only `@tauri-apps/api` import site |