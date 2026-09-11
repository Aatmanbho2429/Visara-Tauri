---
paths:
  - "UI/src/app/core/tauri/**/*.ts"
  - "UI/src-tauri/src/commands/**/*.rs"
  - "UI/src-tauri/src/lib.rs"
---

# Tauri invoke / emit

## Commands resolve through an event, not a return value

`ZoneWrapperService.invoke()` calls `invoke(command, {...args, requestId})` but resolves on a
Tauri **event** named `<command>_response` — not the invoke's own return value. Rust commands emit
that event, typically after spawning async work, rather than returning a value directly.

The client-generated `requestId` is echoed back in the envelope and matched before the Observable
resolves, so a command fired concurrently several times (`browse_get_thumbnail` — the Browse grid
asks for dozens at once) can't have one response satisfy the wrong caller.

**Every command emits its `_response` event.** There is no direct-return exception; the
`requestId` correlation is what makes that safe under concurrency. When adding a command, follow
the emission pattern and thread `request_id: Option<String>` through it.

- Command names: `entity_action` in `snake_case` on the Rust side — `auth_login`, `user_get_list`.
- Keep a single registry of command name strings on the Angular side, `core/tauri/tauri-commands.const.ts`, and reference it everywhere instead of hardcoding strings.
- Same pattern for events: registry at `core/tauri/tauri-events.const.ts`.
- One command per action. Don't multiplex several operations behind one command with a type/action flag.
- New commands are registered in the `invoke_handler![...]` list in `lib.rs` (not `main.rs` —
  this project's `main.rs` is a 3-line entry point that just calls `pictoria_lib::run()`; all
  `tauri::Builder` wiring lives in `lib.rs` so it can also serve the mobile/library target).
- `src-tauri/capabilities/*.json` allowlists **plugin** permissions (dialog, global-shortcut,
  autostart, window, …), not individual `#[tauri::command]`s. A new command needs no capabilities
  entry; a new call into a Tauri *plugin* API does.
- Event payloads are typed `response*` models, never `any`.