# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Pictoria — a desktop app used to search similar tile designs inside a folder locally. Three components:

- **`UI/`** — Angular 21 frontend + Tauri v2 shell (Rust). The Rust side owns *all* business
  logic, the SQLite database, and the vector store; Angular is presentation only, talking to
  Rust exclusively through `invoke()`/events (see "IPC pattern" below).
- **`UI/src-tauri/`** — the Rust backend. Runs image indexing, the SQLite metadata store, the
  custom flat-vector store, folder watching, auth/licensing against Supabase, and spawns/manages
  the Python sidecar.
- **`sidecar/`** — a local-only Flask HTTP service (127.0.0.1, spawned hidden by Rust) that does
  the ML work Rust has no libraries for: DINO embeddings (ONNX), Gabor/Gram-matrix texture
  descriptors, and SIFT/RANSAC geometric verification. It never touches disk state — Rust owns
  every read/write to `meta.db` and `vectors.bin`.


## Commands

All frontend/Tauri commands run from `UI/`.

```bash
# Frontend dev server (Angular only, no Tauri shell)
npm start                      # ng serve, http://localhost:4200

# Full desktop app (spawns sidecar via `python server.py` fallback if no frozen binary staged)
npx tauri dev

# Frontend build
npm run build                  # ng build -> dist/UI/browser

# Frontend tests — Angular's `@angular/build:unit-test` builder, Vitest runner,
# Node + jsdom (no browser). Watch defaults to on in a TTY.
npm test
npm test -- --watch=false
npm test -- --include src/app/views/login/login.spec.ts   # one spec file
npm test -- --filter "^App"                               # one suite/test by regex
npm test -- --list-tests                                  # discover without running

# Rust — from UI/src-tauri/, or `cargo check --manifest-path UI/src-tauri/Cargo.toml`
cargo check
cargo build
cargo test                     # unit tests live in core::database, core::vector_store, services::search::search_service
cargo test vector_store        # one module

# Full desktop build/package
npx tauri build
```

**Before `cargo build`/`cargo check`/`tauri dev` will work**, a frozen sidecar binary must exist
at `UI/src-tauri/binaries/pictoria-sidecar-<host-triple>[.exe]` — Tauri's `externalBin` build
script validates it eagerly and fails with `resource path ... doesn't exist` otherwise. Stage one
locally with (see `sidecar/README.md` for the full explanation):

```bash
cp sidecar/dist/pictoria-sidecar \
   "UI/src-tauri/binaries/pictoria-sidecar-$(rustc -vV | awk '/host:/{print $2}')"
```

`UI/src-tauri/binaries/` is gitignored (~200MB). At runtime in a debug build,
`core::sidecar::spawn()` falls back to `python server.py` if no frozen binary is found — so
`cargo tauri dev` itself doesn't need a fresh freeze, only the build script's existence check does.

Sidecar dev loop (from `sidecar/`):

```bash
pip install -r requirements.txt
python server.py               # first run downloads mobilenet_v2 weights (~14MB)
```

The DINO (embedding) model is encrypted (`clip_vitb32.onnx.enc`, Git LFS) and only loads once
Rust posts a licence-derived key to `POST /model` after a successful login — without it,
`/describe` returns `{"error": "embed-model-not-loaded"}` and indexing/search never start
locally. `GET /health` reports two readiness flags: `ready` (mobilenet/gram, loads at worker
startup) and `embed_ready` (DINO, gated on the key).

Freezing the sidecar into a binary (only needed to test the frozen path, not for normal dev):

```bash
cd sidecar
pyinstaller --onefile --name pictoria-sidecar \
  --add-data "weights/mobilenet_v2-b0353104.pth<SEP>weights" \
  --add-data "clip_vitb32.onnx.enc<SEP>." \
  server.py   # <SEP> is ';' on Windows, ':' elsewhere
```

Releases build via `.github/workflows/release.yml` on `v*` tags — it cross-freezes the sidecar
per-platform (including an x86_64-under-Rosetta leg for Intel macOS, since PyInstaller freezes
whatever interpreter runs it, not a `--target`), code-signs it on macOS before `tauri-action`
seals the `.app`, then builds/publishes the Tauri bundle.

## Architecture

### Rust backend layering (`UI/src-tauri/src/`)

```
commands/  — #[tauri::command] entry points, one file per entity: <entity>_commands.rs. Thin:
             parse args, call a services::* fn, emit `<command>_response` carrying the
             ApiResponse envelope (see IPC pattern below).
services/  — business logic, one folder per entity: services/<entity>/<entity>_service.rs,
             re-exported through services/<entity>/mod.rs so `services::auth::login(...)` etc.
             still resolves without callers caring about the extra directory level. Orchestrates
             core/* primitives and returns ApiResponse<T>; this is where most real work goes.
models/    — the IPC-boundary types: models/request/ and models/response/, one module per
             entity, mirroring UI/src/app/models/ field-for-field (`#[serde(rename_all =
             "camelCase")]` on every struct). ApiResponse<T> lives in
             models/response/api_response.rs.
core/      — low-level primitives with no Tauri dependency: database (SQLite), vector_store
             (custom flat-vector index, replaces FAISS), sidecar (process lifecycle + HTTP
             client to the Python service), watcher (filesystem watching for Watch Folders),
             thumbs, color, search_gate, migrate, progress.
config.rs  — ALL paths, ports, tuning constants, and filesystem-path resolution live here.
             Nothing is hard-coded elsewhere — `use crate::config::*`.
error.rs   — PictoriaError + Result alias used throughout; `.to_response::<T>()` builds the
             error-range ApiResponse<T> straight from a PictoriaError's status_code() mapping.
utils/     — file_utils, image_loader, hotkey (clipboard temp-file path).
lib.rs     — Tauri::Builder wiring only: plugins, tray, global hot-key, invoke_handler list,
             app lifecycle (setup/window-close-to-tray/exit cleanup). No business logic. New
             commands are registered here, not in main.rs.
main.rs    — three lines; just calls `pictoria_lib::run()`.
```

Key invariant: **Rust owns every disk write** to `~/.pictoria/meta.db` (SQLite metadata +
file↔vector-id mapping) and `~/.pictoria/vectors.bin` (flat vector store). The sidecar computes
and returns data over HTTP; it never persists anything itself.

### The sidecar contract (`core::sidecar` ↔ `sidecar/server.py`)

- Spawned hidden at app startup, kept alive for the whole session, watched by a background
  thread that respawns it on crash (bounded — gives up after `MAX_CONSECUTIVE_CRASHES` and
  emits a `sidecar_crashed` event for the UI instead of looping forever).
- Two work queues inside the sidecar, one worker thread: `"search"` priority jobs always run
  before queued `"index"` jobs, so an interactive search never waits behind a whole folder index
  — indexing pauses/resumes around it.
- Endpoints: `GET /health`, `POST /model` (push the decryption key, blocks while
  decrypting+compiling — budget generously, ~seconds to minutes cold), `POST /model/reset`,
  `POST /describe` (batch → embed/rose/gram/color descriptors), `POST /verify` (SIFT/RANSAC
  shortlist verification, parallelized server-side across an 8-way pool).
- The model key lives in memory only on both sides — never written to disk, never logged.
  Dropped on logout or a lapsed subscription.

### Search pipeline (`services::search`)

1. Describe the query image via the sidecar (`/describe`).
2. Select the "near family": every indexed file whose DINO embedding cosine similarity is
   `>= config::NEAR_FAMILY_MIN_SIM` (a floor, not a top-N rank cut — the old fixed 1500-item
   shortlist could silently exclude a real match in a large library). Computed in-process
   against `VectorStore`.
3. Geometrically verify the *entire* near family via the sidecar's `/verify` (SIFT/RANSAC) —
   this is what backs the `verified`/`partial`/`mirrored` flags and match-point count in
   `SearchResult`. Nothing is truncated at this stage either.
4. Secondary Gabor-rose + Gram-matrix score (`ROSE_WEIGHT`/`GRAM_WEIGHT`) is still computed and
   returned but no longer selects candidates — it only breaks ties at equal embedding cosine.

### Licensing / auth flow

Auth and subscription state come from Supabase (`config::SUPABASE_EDGE`). Token-validate returns
an `onnx_key` that `services::auth` hands to `core::sidecar::set_model_key`, which is what
unlocks `/describe`. The bearer token is the only thing persisted to disk
(`~/.pictoria_token`, via OS keychain where available — see `keyring` dependency — falling back
to a plaintext file); it's deleted on logout. `config::OFFLINE_GRACE_SECS` (3 days) is how long a
cached "valid" subscription state survives with no network before forcing a re-validate.

### IPC pattern (Angular ↔ Rust)

Commands are **not** simple request/response. `ZoneWrapperService.invoke()`
([zone-wrapper.service.ts](UI/src/app/core/zone-wrapper/zone-wrapper.service.ts)) calls
`invoke(command, {...args, requestId})` but resolves via a Tauri *event* named
`<command>_response`, not the invoke's own return value — commands emit that event from Rust
(typically after spawning async work) rather than returning a value directly. The client-generated
`requestId` is echoed back in the envelope and matched before the Observable resolves, so a
command fired concurrently more than once (`browse_get_thumbnail` — the Browse grid asks for
dozens at once) can't have one response satisfy the wrong caller. When adding a new Tauri command,
follow this event-emission pattern and thread `request_id: Option<String>` through it.

`ZoneWrapperService` is also the NgZone boundary — it listens outside Angular's zone and re-enters
via `zone.run()` — and the **only** file that imports `@tauri-apps/api` (including
`convertFileSrc`, wrapped as `toAssetUrl()`). Angular services (`auth.service.ts`,
`library.service.ts`, …) inject it and pass command/event names from the registries in
`UI/src/app/core/tauri/` (`tauri-commands.const.ts`, `tauri-events.const.ts`) rather than string
literals.

Five call shapes:

| Method | Use |
|---|---|
| `invoke<T>()` | normal command; shows the global loader; resolves with `data`, errors with `ApiError` on a non-2xx `statusCode` |
| `invokeSilent<T>()` | background/periodic work (e.g. licence re-validation); no loader |
| `invokeFireAndForget()` | a command with no `_response` counterpart — `search_start`/`update_check`/`update_install` report entirely through their own broadcast events |
| `listen<T>()` | subscribe to a broadcast event; returns the full `ApiResponse<T>` envelope, since a stream can carry more than one outcome over its lifetime |
| `toAssetUrl()` | `convertFileSrc` passthrough, so this stays the only `@tauri-apps/api` import site |

Every command/event response carries the same envelope, `ApiResponse<T>`
([api_response.rs](UI/src-tauri/src/models/response/api_response.rs) ↔
[apiResponse.ts](UI/src/app/models/response/apiResponse.ts)):

```ts
{ statusCode: number; message: string; data: T | null; requestId?: string }
```

`PictoriaError::to_response::<T>()` builds the error-range envelope directly from a
`PictoriaError`'s `status_code()` mapping (401 for session errors, 409 `SearchBusy`, 422 image
decode, 503 network/model-not-ready, 500 everything else). Components never check `.success` —
`BaseComponent.handle(obs, onSuccess, onError?)` (or a raw `.subscribe({next, error})`) is how the
error branch is reached, with `onError` receiving the thrown `ApiError`.

Every command emits its `_response` event — there is no more direct-return exception for
`get_thumbnail`/`reset_notice_pending`/`dismiss_reset_notice`; the `requestId` correlation above
is what makes that safe even under the Browse grid's concurrent thumbnail requests.

`UI/src-tauri/capabilities/default.json` lists **plugin** permissions (dialog, global-shortcut,
autostart, window), not commands. A new `#[tauri::command]` needs registering in the
`invoke_handler![]` list in `lib.rs` and nothing else; a new *plugin API* needs a capability entry.

### Angular structure (`UI/src/app/`)

Standalone-components style (Angular 21, PrimeNG UI kit, per-component module imports — no shared
barrel). `views/` are routed pages (login, master shell with search/library/browse/profile
children — see `app.routes.ts`), `services/` (one folder per entity, e.g. `services/auth/`) wrap
Tauri IPC + app state, `guards/` gate routes on auth (`authGuard`/`loginGuard`) and subscription
status (`subscriptionGuard` — profile stays open even when expired, so users can renew). `master`
is the shell layout hosting the gated feature views as router children.

`models/request/` and `models/response/` mirror the Rust `models/` tree field-for-field (both
camelCase); a shared shape (e.g. `requestPath { path }`) is factored into one file and reused
across the commands that take it, rather than one file per literal command.

## Cross-file invariants

Things nothing enforces automatically — get one wrong and it fails at runtime or on a user's
machine, not at build time.

- **App version lives in three places** and they must match:
  `UI/src-tauri/tauri.conf.json` (`version`), `UI/src-tauri/Cargo.toml` (`package.version`),
  and `UI/src-tauri/src/config.rs` (`APP_VERSION`). Currently `1.1.38`.
- **`config::EMBED_DIM` must match the sidecar model's real output width.**
  `sidecar/pipeline.py::load_embed_model` measures it and reports it through `/health`, and
  `core::sidecar` refuses a mismatch rather than let a wrong stride reach `vectors.bin`.
  `EMBED_ZOOM_LEVELS` likewise mirrors `_GRAM_ZOOM_SCALES` in `pipeline.py` — the crops are
  literally shared.
- **Changing how any descriptor is produced invalidates every stored vector.** Bump
  `core::migrate::EMBED_SCHEMA_VERSION` (currently `9`) in the same change. On startup it is
  compared against SQLite `PRAGMA user_version`; a bump deletes `vectors.bin`, drops a
  `.reembed_pending` marker (crash-safe resume), and rebuilds vectors in place against existing
  `files(id)` rows — `files` is never wiped, because Browse's tags cascade off it. The comment
  block above the constant is the changelog of why each version bumped; add to it.
- **`NEAR_FAMILY_MIN_SIM` (0.70) is the only bound on how much work a search does** and is not
  calibrated against the current model. The number to watch is `near_family_n` in the search
  timing log — if it is a large fraction of the library on an ordinary query, the floor is too low.

## Conventions

`.claude/rules/` holds path-scoped rules (globs rooted at `UI/`, so they load automatically when
Claude works with matching files) — all seven now describe the code as it exists.

| Rule file | Covers |
|---|---|
| `code-format.md` | One-line `//` comment style — no JSDoc, no `///`/`//!` on the Rust side |
| `ui-framework.md` | PrimeNG + PrimeIcons usage, per-component module imports |
| `models.md` | `models/request/` + `models/response/` split, camelCase, mirrored trees |
| `services.md` | Per-entity service folders, `ApiResponse<T>` |
| `api-response-format.md` | `{statusCode, message, data}` envelope |
| `tauri-ipc.md` | Command-name const registry, capabilities allowlist, `lib.rs` registration |
| `zone-wrapper.md` | `ZoneWrapperService` as the sole `@tauri-apps/api` import site |

One deliberate, documented simplification versus a literal reading of `models.md`: a request/
response shape shared by several commands (e.g. `requestPath { path }`, used by
`browse_directory`, `library_add_folder`, `library_rescan_folder`, …) lives in one file
(`request_common.rs` / `requestCommon.ts`) rather than being duplicated per command. The "mirrored
trees" invariant still holds — both sides define the same shape under the same name — it's just
not literally one-file-per-command where the command's own shape adds nothing.

The `/scaffold-entity` skill (`.claude/skills/scaffold-entity/`) walks the same structure new
entities should follow.
