# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

<!-- Keep this file under ~200 lines. Anything that only matters in one part of the tree belongs
     in a path-scoped rule under .claude/rules/, which loads on demand; anything that's a
     multi-step procedure belongs in a skill. See the Conventions section. -->

## What this is

Pictoria — a desktop app for searching similar tile designs in a local folder. Three components:

- **`UI/`** — Angular 21 frontend. Presentation only; talks to Rust exclusively through
  `invoke()`/events.
- **`UI/src-tauri/`** — Tauri v2 Rust backend. Owns *all* business logic: image indexing, the
  SQLite metadata store, the vector store, folder watching, auth/licensing, and the sidecar
  process.
- **`sidecar/`** — a local-only Flask service (127.0.0.1, spawned hidden by Rust) doing the ML work
  Rust has no libraries for: DINO embeddings (ONNX), Gabor/Gram texture descriptors, SIFT/RANSAC
  geometric verification.

**Rust owns every disk write** to `~/.pictoria/meta.db` and `~/.pictoria/vectors.bin`. The sidecar
computes and returns data over HTTP; it never persists anything.

## Commands

From `UI/`:

```bash
npm start                      # Angular only, http://localhost:4200
npx tauri dev                  # full desktop app
npm run build                  # -> dist/UI/browser

npm test                       # Vitest via @angular/build:unit-test, node+jsdom; watches in a TTY
npm test -- --watch=false
npm test -- --include src/app/views/login/login.spec.ts   # one spec file
npm test -- --filter "^App"                               # one suite/test by regex
npm test -- --list-tests                                  # discover without running

npx tauri build
```

From `UI/src-tauri/`, or add `--manifest-path UI/src-tauri/Cargo.toml`:

```bash
cargo check
cargo test                     # core::database, core::vector_store,
                               # services::search::search_service, models::response serde
cargo test vector_store        # one module
```

**`cargo check`/`build`/`tauri dev` fail until a frozen sidecar binary exists** at
`UI/src-tauri/binaries/pictoria-sidecar-<host-triple>[.exe]` — Tauri's `externalBin` validates it
eagerly and fails with `resource path ... doesn't exist`. That directory is gitignored (~200MB).
Stage one:

```bash
cp sidecar/dist/pictoria-sidecar \
   "UI/src-tauri/binaries/pictoria-sidecar-$(rustc -vV | awk '/host:/{print $2}')"
```

Only the build script's existence check needs it — at runtime a debug build falls back to
`python server.py`. Sidecar dev loop and freezing: `.claude/rules/sidecar.md`, `sidecar/README.md`.

## Cross-file invariants

Nothing enforces these automatically. Get one wrong and it fails at runtime or on a user's
machine, not at build time.

- **App version lives in three places** and they must match: `UI/src-tauri/tauri.conf.json`
  (`version`), `UI/src-tauri/Cargo.toml` (`package.version`), and `UI/src-tauri/src/config.rs`
  (`APP_VERSION`). Currently `1.1.40`.
- **`config::EMBED_DIM` must match the sidecar model's real output width.**
  `sidecar/pipeline.py::load_embed_model` measures it and reports it through `/health`, and
  `core::sidecar` refuses a mismatch rather than let a wrong stride reach `vectors.bin`.
  `EMBED_ZOOM_LEVELS` likewise mirrors `_GRAM_ZOOM_SCALES` in `pipeline.py` — the crops are
  literally shared.
- **Changing how any descriptor is produced invalidates every stored vector.** Bump
  `core::migrate::EMBED_SCHEMA_VERSION` (currently `9`) in the same change. On startup it's
  compared against SQLite `PRAGMA user_version`; a bump deletes `vectors.bin`, drops a
  `.reembed_pending` marker (crash-safe resume), and rebuilds vectors in place against existing
  `files(id)` rows — `files` is never wiped, because Browse's tags cascade off it. The comment
  block above the constant is the changelog of why each version bumped; add to it.
- **`NEAR_FAMILY_MIN_SIM` (0.70) is the only bound on how much work a search does** and is not
  calibrated against the current model. Watch `near_family_n` in the search timing log — if it's a
  large fraction of the library on an ordinary query, the floor is too low.
- **`config.rs` holds every path, port, and tuning constant.** Nothing is hard-coded elsewhere.

## Diagnosing behaviour at runtime

`tauri_plugin_log` is configured in `lib.rs` at Info (Debug in debug builds) to stdout and LogDir,
with `max_file_size` at 5 MB and `KeepOne` rotation — the plugin's 40 KB default rotates away the
instrumentation in seconds.

- Windows: `%APPDATA%\com.pictoria.app\logs\Pictoria.log` (filename is `productName`)
- macOS: `~/Library/Logs/com.pictoria.app/Pictoria.log`

Every expensive path emits a one-line `[timing]` record, in release too. `[timing] search TOTAL`
carries the per-stage breakdown (`near_family_n`, `verified_n`, `describe_ms`, `near_family_ms`,
`verify_ms`, `total_ms`) and is the only way to tell whether a slow search is the sidecar, the
near-family scan, or SIFT; `sync_folder TOTAL`/`chunk`/`scan_images` do the same for indexing.
**Quote a real log line rather than an estimate when reporting a timing claim.**

## Plan and reference documents

Phased, checkbox-tracked plans that set their own working rules. Read the plan before working it,
and trust its own checklist over any summary here.

- `docs/plans/image-quota.md` — per-plan library-size metering; the active `Library_based_price`
  work. Core anti-gaming rule: the real ledger has one birth event per image and is never
  decremented.
- `docs/plans/annual-monthly-billing.md` — annual (₹8,000 × 12) and quarterly (₹8,333 × 3) debited
  monthly via Razorpay Subscriptions, sized to stay under RBI's ₹15,000 no-AFA e-mandate ceiling.
  Approved, not started. Introduces the project's first webhook endpoint; its governing invariant
  is "fail closed by time, fail open by webhook", and because force-update leaves no client
  rollback, the only kill switch is the `plans.is_active` row inserted last.
- `docs/plans/force-update.md` — mandatory-update overlay. Phases 1–3 shipped, phase 4 (server-
  issued `minVersion` kill switch) deferred. Forcing is gated on `!isDevMode()`, so `tauri dev` is
  never blocked; nothing can switch it *off* in a shipped build.
- `docs/backend/` — the Supabase project: schema, RLS, and all 20 deployed edge functions against
  the 11 the app calls. Read `docs/backend/README.md` before touching auth, subscription, or
  billing — it flags several load-bearing footguns.
- `SEARCH-LATENCY-PLAN.md`, `MIGRATION-PLAN.md` (root) — both fully landed. Historical record;
  nothing pending.

## Conventions

`.claude/rules/` holds path-scoped rules that load automatically when Claude reads a matching file,
so that's where anything specific to one part of the tree lives — architecture included. Check for
a relevant rule before re-deriving how a layer works:

| Rule | Scope |
|---|---|
| `code-format.md` | Comment style. One line, never a paragraph. Always loaded |
| `rust-layering.md` | `commands/` → `services/` → `core/` layering, `config.rs`, `error.rs` |
| `sidecar.md` | Sidecar HTTP contract, queues, model key, freeze/build |
| `search-pipeline.md` | The four search stages and the near-family floor |
| `auth-licensing.md` | Supabase auth/subscription flow and the backend footguns |
| `tauri-ipc.md` | `<command>_response` event pattern, `requestId`, command registry |
| `zone-wrapper.md` | `ZoneWrapperService` as sole `@tauri-apps/api` import site; five call shapes |
| `api-response-format.md` | `{statusCode, message, data}` envelope and its error branch |
| `models.md` | `request/` + `response/` split, camelCase, mirrored Rust/TS trees |
| `services.md` | Per-entity service folders |
| `ui-framework.md` | Angular structure, PrimeNG usage |

`/scaffold-entity` walks the structure a new entity should follow.

One documented simplification versus a literal reading of `models.md`: a request/response shape
shared by several commands (e.g. `requestPath { path }`, used by `browse_directory`,
`library_add_folder`, `library_rescan_folder`, …) lives in one file (`request_common.rs` /
`requestCommon.ts`) rather than being duplicated per command. Both sides still define the same
shape under the same name.
