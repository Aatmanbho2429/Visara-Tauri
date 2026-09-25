---
paths:
  - "UI/src-tauri/src/**/*.rs"
---

# Rust backend layering (`UI/src-tauri/src/`)

Work with the grain of these layers — most new work belongs in `services/`.

- **`commands/`** — `#[tauri::command]` entry points, one file per entity
  (`<entity>_commands.rs`). Thin: parse args, call a `services::*` fn, emit the
  `<command>_response` event carrying the `ApiResponse` envelope.
- **`services/`** — business logic, one folder per entity
  (`services/<entity>/<entity>_service.rs`), re-exported through `mod.rs` so
  `services::auth::login(...)` resolves without callers knowing about the extra level.
  Orchestrates `core/*` primitives and returns `ApiResponse<T>`.
- **`models/`** — the IPC-boundary types only, split `request/` and `response/`, mirroring
  `UI/src/app/models/` field-for-field with `#[serde(rename_all = "camelCase")]` on every struct.
- **`core/`** — low-level primitives with no Tauri dependency: `database` (SQLite), `vector_store`
  (custom flat-vector index, replaces FAISS), `sidecar`, `watcher`, `thumbs`, `color`,
  `search_gate`, `migrate`, `progress`.
- **`config.rs`** — ALL paths, ports, tuning constants, and path resolution. Nothing is hard-coded
  elsewhere; `use crate::config::*`.
- **`error.rs`** — `PictoriaError` + `Result` alias. `.to_response::<T>()` builds the error-range
  `ApiResponse<T>` from the `status_code()` mapping: 401 session, 409 `SearchBusy`, 422 image
  decode, 503 network/model-not-ready, 500 everything else.
- **`lib.rs`** — `tauri::Builder` wiring only: plugins, tray, global hot-key, `invoke_handler`
  list, app lifecycle. No business logic. `main.rs` is three lines calling `pictoria_lib::run()`.
- **`utils/`** — `file_utils`, `image_loader`, `hotkey`.

Two folders under `services/` are not UI entities:

- **`sync`** — the folder-indexing pipeline: scan → hash/dedupe against SQLite → sidecar
  `/describe` in `INDEX_FILE_CHUNK`-sized (64) chunks → vector store.
- **`license`** — device fingerprinting (see `auth-licensing.md`).

## Invariant

**Rust owns every disk write** to `~/.pictoria/meta.db` (SQLite metadata plus the file↔vector-id
mapping) and `~/.pictoria/vectors.bin` (flat vector store). The sidecar returns data over HTTP and
persists nothing.
