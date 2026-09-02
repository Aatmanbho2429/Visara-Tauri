---
paths:
  - "src/app/core/tauri/**/*.ts"
  - "src-tauri/src/commands/**/*.rs"
  - "src-tauri/src/main.rs"
---

# Tauri invoke / emit

- Command names: `entity_action` in `snake_case` on the Rust side — `auth_login`, `user_get_list`.
- Keep a single registry of command name strings on the Angular side, `core/tauri/tauri-commands.const.ts`, and reference it everywhere instead of hardcoding strings.
- Same pattern for events: registry at `core/tauri/tauri-events.const.ts`.
- One command per action. Don't multiplex several operations behind one command with a type/action flag.
- Every new command needs an entry in the relevant Tauri v2 capabilities allowlist (`src-tauri/capabilities/*.json`) — add this in the same change, not a follow-up, or the command will be silently denied at runtime.
- Event payloads are typed `response*` models, never `any`.