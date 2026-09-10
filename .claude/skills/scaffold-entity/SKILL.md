---
name: scaffold-entity
description: Scaffold a new entity end-to-end — Angular request/response models, an Angular service, Tauri request/response models, a Tauri service, and Tauri commands — following this project's conventions. Use when the user asks to add a new entity, resource, or API endpoint, e.g. "add a product entity" or "wire up a new users API".
argument-hint: [entityName]
---

# Scaffold a new entity: $ARGUMENTS

Create every piece needed for a new entity called `$ARGUMENTS`, following the conventions in `.claude/rules/`.

## Steps

1. **Angular request/response models** — `src/app/models/request/request<EntityName>.ts` and `src/app/models/response/response<EntityName>.ts`, camelCase fields. See `models.md`.
2. **Angular service** — `src/app/services/<entity>/<entity>.service.ts`. Inject `ZoneWrapperService`; every method calls `zoneWrapper.invoke()`, never `invoke` directly. See `zone-wrapper.md`.
3. **Tauri command name constants** — add entries to `src/app/core/tauri/tauri-commands.const.ts` for each new command, `entity_action` snake_case. See `tauri-ipc.md`.
4. **Rust request/response models** — `src-tauri/src/models/request/request_<entity>.rs` and `.../response/response_<entity>.rs`, mirroring the TS models field-for-field, with `#[serde(rename_all = "camelCase")]`. See `models.md`.
5. **Rust service** — `src-tauri/src/services/<entity>/<entity>_service.rs` holding the actual logic, returning `ApiResponse<T>`.
6. **Rust command handlers** — `src-tauri/src/commands/<entity>_commands.rs`, thin `#[tauri::command]` functions that delegate to the service.
7. **Register commands** — add each new command to the `invoke_handler![...]` list in `lib.rs` (not `main.rs`, which is just a 3-line entry point).
8. **Capabilities** — only needed if the entity calls a Tauri *plugin* API (dialog, global-shortcut, autostart, …); add it to the relevant `src-tauri/capabilities/*.json` allowlist. Plain `#[tauri::command]`s need no capabilities entry.
9. **Comments** — a single `//` line above every new function and non-obvious variable. See `code-format.md`.

## After scaffolding

Confirm both trees mirror each other by entity name, and that no service imports `@tauri-apps/api` directly — only `zone-wrapper.service.ts` should.