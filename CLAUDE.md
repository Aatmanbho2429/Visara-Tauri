# CLAUDE.md
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

# Frontend tests (Vitest)
npm test

# Rust — from UI/src-tauri/, or `cargo check --manifest-path UI/src-tauri/Cargo.toml`
cargo check
cargo build
cargo test

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
commands/  — #[tauri::command] entry points. Thin: parse args, call a services::* fn, emit
             a `<command>_response` event (see IPC pattern below). One file per feature area.
services/  — business logic: auth, browse, library, license, search, subscription, sync, tags.
             Orchestrate core/* primitives; this is where most real work should be added.
core/      — low-level primitives with no Tauri dependency: database (SQLite), vector_store
             (custom flat-vector index, replaces FAISS), sidecar (process lifecycle + HTTP
             client to the Python service), watcher (filesystem watching for Watch Folders),
             thumbs, color, search_gate, migrate, progress.
config.rs  — ALL paths, ports, tuning constants, and filesystem-path resolution live here.
             Nothing is hard-coded elsewhere — `use crate::config::*`.
error.rs   — PictoriaError + Result alias used throughout.
utils/     — file_utils, image_loader.
lib.rs     — Tauri::Builder wiring only: plugins, tray, global hot-key, invoke_handler list,
             app lifecycle (setup/window-close-to-tray/exit cleanup). No business logic.
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

Commands are **not** simple request/response. `TauriService.invoke()` (`UI/src/app/services/
tauri.service.ts`) calls `invoke(command, args)` but resolves via a Tauri *event* named
`<command>_response`, not the invoke's own return value — commands emit that event from Rust
(typically after spawning async work) rather than returning a value directly. Long-running
operations (search, library sync) use their own dedicated event streams instead
(`search_progress`/`search_complete`/`search_error`, `library_sync_started/progress/complete/
error`) via `searchStream()` / `onLibrarySync()`. When adding a new Tauri command, follow this
event-emission pattern rather than returning data straight from the `#[tauri::command]` fn.

### Angular structure (`UI/src/app/`)

Standalone-components style (Angular 21, PrimeNG UI kit). `views/` are routed pages (login,
master shell with search/library/browse/profile children — see `app.routes.ts`), `services/`
wrap Tauri IPC + app state, `guards/` gate routes on auth (`authGuard`/`loginGuard`) and
subscription status (`subscriptionGuard` — profile stays open even when expired, so users can
renew). `master` is the shell layout hosting the gated feature views as router children.

## Conventions
Detailed, path-scoped rules live in `.claude/rules/` and load automatically when Claude works with matching files — they don't need to be repeated here:
 
| Rule file | Covers |
|---|---|
| `ui-framework.md` | PrimeNG usage |
| `models.md` | Request/Response model conventions |
| `services.md` | Entity-based service structure |
| `api-response-format.md` | The `{statusCode, message, data}` envelope |
| `tauri-ipc.md` | `invoke` / `emit` naming and wiring |
| `zone-wrapper.md` | Routing every Tauri call through `ZoneWrapperService` |
| `code-comments.md` | One-line comment style |
 
There's also a `/scaffold-entity` skill (`.claude/skills/scaffold-entity/`) that walks through adding a new entity end-to-end following all of the above.
