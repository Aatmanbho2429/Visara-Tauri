# Conventions migration plan

Bring the codebase in line with `.claude/rules/` and `.claude/skills/scaffold-entity/`.

Today five of the seven rule files describe a structure the tree was never migrated to. This
plan closes that gap. Work the phases in order — each one compiles and runs on its own, so the
migration can be paused between phases without leaving the app broken.

## Locked decisions

| # | Decision | Chosen |
|---|---|---|
| 1 | Response envelope | **Full switch to `{statusCode, message, data}`.** `BaseResponse` / `success` is deleted. |
| 2 | Rust comment style | **Convert all 902 `///` and `//!` lines to `//`.** No doc-comments anywhere. |
| 3 | Command registration | **Fix the rule, keep the code.** Commands stay wired in `lib.rs`; `tauri-ipc.md` + `SKILL.md` are corrected. |
| 4 | Stream / direct-return commands | **Force everything into the envelope.** Streams get wrapped; the four direct-return commands emit `<command>_response`. |

### Note on decision 4

`get_thumbnail`'s existing comment warns that a shared `<command>_response` event cross-wires the
dozens of concurrent calls the Browse grid fires. That warning is correct, and moving it onto the
event pattern reintroduces the bug **unless** responses are correlated.

Phase 5 therefore builds correlation into `ZoneWrapperService` generically: every invoke gets a
`requestId`, Rust echoes it in the envelope, and the client ignores any `<command>_response` whose
`requestId` doesn't match. This makes concurrent same-command calls safe for *every* command, not
just thumbnails. **Do not skip it** — it is what makes decision 4 shippable.

---

## Phase 0 — Fix the rules so they load at all

Nothing else works until this is done: every rule's `paths:` glob is rooted at `src/` and
`src-tauri/`, but the real tree lives under `UI/`. **No rule currently matches any file.**

**`.claude/rules/*.md` — prefix every glob with `UI/`:**

| File | Current | Corrected |
|---|---|---|
| `models.md` | `src/app/models/**/*.ts` | `UI/src/app/models/**/*.ts` |
| | `src-tauri/src/models/**/*.rs` | `UI/src-tauri/src/models/**/*.rs` |
| `services.md` | `src/app/services/**/*.ts` | `UI/src/app/services/**/*.ts` |
| | `src-tauri/src/services/**/*.rs` | `UI/src-tauri/src/services/**/*.rs` |
| `api-response-format.md` | `src/app/models/response/**/*.ts` | `UI/src/app/models/response/**/*.ts` |
| | `src-tauri/src/models/response/**/*.rs` | `UI/src-tauri/src/models/response/**/*.rs` |
| | `src-tauri/src/commands/**/*.rs` | `UI/src-tauri/src/commands/**/*.rs` |
| `tauri-ipc.md` | `src/app/core/tauri/**/*.ts` | `UI/src/app/core/tauri/**/*.ts` |
| | `src-tauri/src/commands/**/*.rs` | `UI/src-tauri/src/commands/**/*.rs` |
| | `src-tauri/src/main.rs` | `UI/src-tauri/src/lib.rs` |
| `zone-wrapper.md` | `src/app/services/**/*.ts` | `UI/src/app/services/**/*.ts` |
| | `src/app/core/zone-wrapper/**/*.ts` | `UI/src/app/core/zone-wrapper/**/*.ts` |
| `ui-framework.md` | `src/**/*.ts`, `src/**/*.html` | `UI/src/**/*.ts`, `UI/src/**/*.html` |

**Content corrections (decision 3):**

- `tauri-ipc.md` — replace "add each new command to the `invoke_handler![...]` list in `main.rs`"
  with `lib.rs`. Replace the capabilities bullet: in Tauri v2 `capabilities/*.json` gates **plugin
  permissions** (dialog, global-shortcut, autostart, window), not individual commands; a new
  `#[tauri::command]` needs no capability entry, a new *plugin API* does.
- `.claude/skills/scaffold-entity/SKILL.md` — step 7 says `main.rs`; change to `lib.rs`. Step 8's
  capabilities instruction gets the same correction. Step 9 references `code-comments.md`; the
  file is `code-format.md`.

**Verify:** open a file under `UI/src/app/services/` and confirm the rule now loads.

---

## Phase 1 — Rust `ApiResponse` + models tree

### 1a. The envelope

Create `UI/src-tauri/src/models/mod.rs`, `models/request/mod.rs`, `models/response/mod.rs`, and
register `pub mod models;` in `lib.rs`.

`UI/src-tauri/src/models/response/api_response.rs`:

```rust
// Uniform envelope for every command response crossing the IPC boundary.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiResponse<T> {
    pub status_code: u16,
    pub message: String,
    pub data: Option<T>,
    // Echoes the client's correlation id so concurrent calls to one command
    // can't consume each other's response event. See Phase 5.
    pub request_id: Option<String>,
}
```

Give it `ok(data)`, `ok_with_message(msg, data)`, and `err(status_code, message)` constructors.

### 1b. Error → status code mapping

Replace `PictoriaError::to_json()` in `UI/src-tauri/src/error.rs` with `status_code()` plus a
`to_response<T>()` that builds `ApiResponse`:

| Variant | statusCode |
|---|---|
| `NoSession`, `SessionExpired` | 401 |
| `Image` | 422 |
| `SearchBusy` | 409 |
| `Network` | 503 |
| `ModelNotReady` | 503 |
| `Io`, `Database`, `Fatal` | 500 |
| success | 200 |

### 1c. Request/response models — one file per command

`models/request/request_<entity>_<action>.rs` → `struct Request<Entity><Action>`;
`models/response/response_<entity>_<action>.rs` → `struct Response<Entity><Action>`.
Every struct carries `#[serde(rename_all = "camelCase")]`.

Move the three existing IPC structs out of `services/`: `SearchResult`, `MatchRegion`,
`FailedFile` (`services/search.rs`), `FileError` (`services/sync.rs`) → `models/response/`.

Everything else is currently built ad-hoc with `serde_json::json!` in nine files
(`commands/{auth,search,tags}.rs`, `services/{auth,browse,library,subscription,tags}.rs`,
`error.rs`). Each of those inline shapes becomes a typed response model. Full command inventory
in the appendix.

**Verify:** `cargo check` (models compile standalone before anything consumes them).

---

## Phase 2 — Rust services → per-entity folders

`services.md` requires `services/<entity>/<entity>_service.rs` returning `ApiResponse<T>`.

```
services/auth.rs          → services/auth/auth_service.rs
services/browse.rs        → services/browse/browse_service.rs
services/library.rs       → services/library/library_service.rs
services/license.rs       → services/license/license_service.rs
services/search.rs        → services/search/search_service.rs
services/subscription.rs  → services/subscription/subscription_service.rs
services/sync.rs          → services/sync/sync_service.rs
services/tags.rs          → services/tags/tags_service.rs
```

Each entity folder gets a `mod.rs` re-exporting its service; `services/mod.rs` lists the entities.

Change every service function that currently returns `serde_json::Value` or
`Result<T, PictoriaError>` to return `ApiResponse<T>` built from the Phase 1 models. Errors are
caught **here**, not in the command layer — `api-response-format.md`: commands never panic or
return a raw error string.

`services/search/search_service.rs` keeps its `cargo test` unit tests (3 of them) — move them
with the file and confirm they still pass.

**Verify:** `cargo test` — the 3 search tests plus 6 database and 9 vector_store tests.

---

## Phase 3 — Rust commands: rename, restructure, envelope

### 3a. File renames

`commands/<entity>.rs` → `commands/<entity>_commands.rs` for all eight; update `commands/mod.rs`
and the `use commands::{...}` line in `lib.rs`. `commands/hotkey.rs` holds no `#[tauri::command]`
(only `temp_path()`) — move it to `utils/hotkey.rs`.

### 3b. Command renames to `entity_action`

Eleven commands violate the naming rule:

| Current | New |
|---|---|
| `start_search` | `search_start` |
| `get_plans` | `subscription_get_plans` |
| `get_user_subscriptions` | `subscription_get_user_subscriptions` |
| `create_order` | `subscription_create_order` |
| `verify_payment` | `subscription_verify_payment` |
| `reset_notice_pending` | `notice_reset_pending` |
| `dismiss_reset_notice` | `notice_dismiss_reset` |
| `check_for_update` | `update_check` |
| `install_update` | `update_install` |
| `get_thumbnail` | `browse_get_thumbnail` |
| `open_file_path` | `file_open_path` |

`sidecar_status` already conforms (entity `sidecar`, action `status`) — leave it.
The 10 `auth_*`, 7 `library_*`, 7 `tags_*` and `browse_directory` already conform.

Each rename changes the emitted event name too (`<command>_response`).

### 3c. Envelope every command (decision 4)

All 37 commands become thin: parse args into a `Request*` model, call the service, emit
`<command>_response` carrying `ApiResponse<T>`. Include the `requestId` the client passed.

The four direct-return commands convert to the event pattern:

- `browse_get_thumbnail` — **must** carry `requestId`; this is the concurrency case.
- `notice_reset_pending`, `notice_dismiss_reset`, `file_open_path` — straightforward.

Delete the "returns the value directly" comments; they no longer describe the code.

### 3d. Wrap stream payloads

`search_progress` / `search_complete` / `search_error` and the four `library_sync_*` events all
emit their payload bare today. Wrap each in `ApiResponse<T>` with a typed `Response*` model.
`search_error` and `library_sync_error` carry an error-range `statusCode`; progress and complete
carry 200. These are broadcast events, so they carry no `requestId`.

Also wrap: `update_available`, `update_progress`, `update_error`, `update_not_available`,
`hotkey_pressed`, `sidecar_crashed`, `tags_updated`.

**Verify:** `cargo check && cargo test`, then `npx tauri dev` — the app will not function until
Phase 6 lands, but it must build and launch.

---

## Phase 4 — Angular models tree

Create `UI/src/app/models/request/` and `UI/src/app/models/response/`.

- `requestLogin.ts` → `export interface requestLogin`, `responseLogin.ts` → `responseLogin`.
  Names are camelCase, prefixed `request`/`response`, mirroring the Rust files one-for-one.
- `models/response/apiResponse.ts` replaces `base-response.model.ts`:
  `{ statusCode: number; message: string; data: T | null; requestId?: string }`.
- Split the existing flat models — `auth.model.ts` (8 interfaces), `browse.model.ts` (9),
  `library.model.ts` (6) — into per-command request/response files.
- Move the 12 interfaces currently declared inside services (`auth.service.ts`'s `LoginPayload` /
  `RegisterPayload`; `tauri.service.ts`'s `UpdateInfo`, `UpdateProgress`, `HotkeyEvent`,
  `LibrarySync*`, `SearchEvent`, `SidecarCrashedEvent`) into `models/`.

### camelCase field renames

The wire format is snake_case today. `models.md` requires camelCase on both sides; the Rust
`#[serde(rename_all = "camelCase")]` from Phase 1 changes the wire, so the TS side must follow.
Nineteen fields, each touching 1–5 files:

`first_name`→`firstName`, `last_name`→`lastName`, `company_name`→`companyName`,
`phone_number`→`phoneNumber`, `days_remaining`→`daysRemaining`,
`subscription_status`→`subscriptionStatus`, `subscription_end`→`subscriptionEnd`,
`start_date`→`startDate`, `end_date`→`endDate`, `created_at`→`createdAt`, `added_at`→`addedAt`,
`image_count`→`imageCount`, `total_indexed_files`→`totalIndexedFiles`,
`watched_folder_count`→`watchedFolderCount`, `last_event_at`→`lastEventAt`,
`payment_method`→`paymentMethod`, `razorpay_payment_id`→`razorpayPaymentId`,
`has_image`→`hasImage`, `image_path`→`imagePath`.

Update the HTML templates that bind them. Grep is safe here — the only snake_case in templates
besides these is BEM CSS classes (`card__actions`), which use a double underscore.

---

## Phase 5 — `ZoneWrapperService` + IPC registries

### 5a. The zone wrapper

Create `UI/src/app/core/zone-wrapper/zone-wrapper.service.ts`. It absorbs everything
`tauri.service.ts` does today and becomes **the only file in the app importing
`@tauri-apps/api`** — eight files import it now (`app.ts`, `browse.service.ts`,
`search-state.service.ts`, `tauri.service.ts`, `update.service.ts`, `views/browse.ts`,
`views/library.ts`, `views/search.ts`); all seven others must go through the wrapper.

API:

- `invoke<T>(command, args?): Observable<T>` — generates a `requestId`, passes it as an arg,
  listens for `<command>_response`, **filters on matching `requestId`**, unwraps `.data`, and
  re-enters `NgZone`. Callers never see the envelope.
- `invokeSilent<T>(...)` — same, no global loader.
- `listen<T>(event): Observable<T>` — for the broadcast streams; unwraps the envelope.

**Error channel.** `zone-wrapper.md` says callers only ever see the plain payload, so a non-2xx
`statusCode` must surface as an RxJS `error` carrying `{ statusCode, message }`. This replaces all
**33 `.success` checks** across the app: `if (res.success)` becomes the `next` handler, and the
`else` branch becomes an `error` handler or `catchError`. Walk every call site — a silently
dropped error branch is the main regression risk in this phase.

Then delete `tauri.service.ts`.

### 5b. Name registries

`UI/src/app/core/tauri/tauri-commands.const.ts` — all 37 command names as consts.
`UI/src/app/core/tauri/tauri-events.const.ts` — every event name: the 28 `<command>_response`
strings plus `search_progress`, `search_complete`, `search_error`, `library_sync_started`,
`library_sync_progress`, `library_sync_complete`, `library_sync_error`, `update_available`,
`update_progress`, `update_error`, `update_not_available`, `hotkey_pressed`, `sidecar_crashed`,
`tags_updated`.

No string literal command or event names anywhere else after this phase.

---

## Phase 6 — Angular services → per-entity folders

```
services/auth.service.ts          → services/auth/auth.service.ts
services/browse.service.ts        → services/browse/browse.service.ts
services/library.service.ts       → services/library/library.service.ts
services/tags.service.ts          → services/tags/tags.service.ts
services/update.service.ts        → services/update/update.service.ts
services/search-state.service.ts  → services/search/search-state.service.ts
services/user-state.service.ts    → services/user/user-state.service.ts
services/loader.service.ts        → services/loader/loader.service.ts
services/config.service.ts        → services/config/config.service.ts
```

Every method: inject `ZoneWrapperService`, call `zoneWrapper.invoke(TAURI_COMMANDS.X, req)`,
return `Observable<response*>` — no envelope in any signature. Components never call `invoke`
directly (`views/browse.ts`, `views/library.ts`, `views/search.ts` do today — route them through
their entity service).

**Delete `services/api.service.ts`** — it is dead code (nothing imports it) and its
`ConfigService.apiBaseUrl` (`http://127.0.0.1:8765/api/v1`) is a leftover from the retired Python
backend. Keep `ConfigService` for `isMaintenance` / `setTimeMinutes`, drop `apiBaseUrl`.

---

## Phase 7 — PrimeNG per-component imports

`ui-framework.md`: import PrimeNG modules per standalone component, not through a shared barrel.

Delete `UI/src/app/shared/primeng-components-module.ts` (a 28-module barrel). Four components
import it — `views/login.ts`, `views/profile.ts`, `views/search.ts`,
`shared/plans-dialog/plans-dialog.ts`. For each, read the template and import only the modules it
actually uses. This is the phase that shrinks the bundle; expect most components to need 4–8
modules rather than 28.

Confirm the PrimeNG theme/preset config is centralised in `app.config.ts` (it appears to be —
verify and leave a note if any component overrides it locally).

---

## Phase 8 — Comment style sweep

`code-format.md`: one `//` line above each function and non-obvious variable. No `/** */`, no
multi-line blocks, no `///` or `//!` (decision 2 — convert all of them).

- **TypeScript:** 89 JSDoc blocks. Heaviest: `views/search.ts` (22), `views/library.ts` (13),
  `services/{update,tauri,auth}.ts` (6 each), `models/browse.model.ts` (6).
- **Rust:** 902 lines — 666 `///` and 236 `//!` across 28 files. Heaviest: `core/sidecar.rs` (161),
  `core/vector_store.rs` (77), `core/migrate.rs` (76), `core/watcher.rs` (66),
  `utils/image_loader.rs` (58), `services/search.rs` (57), `core/database.rs` (54),
  `config.rs` (53).

This is a mechanical conversion, but **preserve the content** — several of these blocks are the
only record of why the system works the way it does (the `EMBED_SCHEMA_VERSION` v2→v9 changelog in
`migrate.rs`, the `vectors.bin` binary layout in `vector_store.rs`, the crash-recovery contract in
`sidecar.rs`). Convert the marker, keep the prose. Where a block exceeds a few lines, keep it as
consecutive `//` lines rather than compressing it away.

Do this phase **last among the code phases** — it touches nearly every file and would collide with
every earlier diff.

---

## Phase 9 — Update the docs

- `CLAUDE.md` — delete the "aspirational / matches the codebase" table and the divergence list;
  every row now matches. Update the IPC section: `ApiResponse {statusCode, message, data}`,
  `ZoneWrapperService`, the registries, no direct-return exceptions.
- `.claude/skills/scaffold-entity/SKILL.md` — verify each of its 9 steps now describes something
  that exists. It should need only the Phase 0 corrections.

---

## Verification gates

| After | Command |
|---|---|
| Phase 1–3 | `cargo check && cargo test` (18 tests: 6 database, 9 vector_store, 3 search) |
| Phase 4–7 | `npm test -- --watch=false` and `npm run build` |
| Phase 7 | Check the production bundle budget — `angular.json` errors above 4MB initial |
| End | `npx tauri dev`, then exercise: login → add a watch folder → index → search → tag in Browse → profile/subscription |

The end-to-end pass matters more than the unit tests here. Only 5 spec files exist
(`app`, `login`, `register`, `master`, `search`, `contact-us`) and they cover very little of what
this migration touches.

---

## Risk notes

1. **The envelope switch is the whole-app breaking change.** Between Phase 3 and Phase 6 the Rust
   and Angular sides disagree — the app builds but does not work. Land Phases 3–6 together, or on
   a branch, rather than shipping between them.
2. **The 33 `.success` call sites are the regression surface.** Each becomes an error handler; a
   missed one silently swallows a failure that used to show a message.
3. **`get_thumbnail` correlation is not optional** (see decision 4 note).
4. **Streaming search is the hardest thing to smoke-test** — it needs a licensed login and an
   indexed folder, because the DINO model only loads after `POST /model` succeeds. Budget time for
   it and don't assume a compiling build means a working search.
5. Phase 8 touching 900+ lines will conflict with anything left uncommitted from earlier phases.

---

## Appendix — command inventory (37)

**auth** (10, all conform): `auth_login(email, password)`, `auth_validate_token()`,
`auth_periodic_revalidate()`, `auth_check_session()`, `auth_logout()`, `auth_send_otp(email)`,
`auth_forgot_password_send_otp(email)`, `auth_forgot_password_verify_otp(email, otp_code)`,
`auth_change_password(old_password, new_password)`, `auth_request_access(...)`

**library** (7, all conform): `library_list_folders()`, `library_add_folder(path)`,
`library_remove_folder(path, purge)`, `library_set_paused(path, paused)`,
`library_rescan_folder(path)`, `library_stats()`, `library_folder_tree(path)`

**tags** (7, all conform): `tags_set(paths, category, value)`, `tags_remove(paths, category,
value)`, `tags_get(paths)`, `tags_facets()`, `tags_query(filters)`, `tags_suggest(paths)`,
`tags_backfill_colors()`

**subscription** (4, all rename): `get_plans()`, `get_user_subscriptions()`,
`create_order(user_id, plan_id)`, `verify_payment(...)`

**search** (2): `start_search(...)` → rename; `sidecar_status()` conforms

**browse** (2): `browse_directory(path)` conforms; `get_thumbnail(path)` → rename + correlation

**update** (2, both rename): `check_for_update()`, `install_update()`

**notice** (2, both rename): `reset_notice_pending()`, `dismiss_reset_notice()`

**misc** (1, rename): `open_file_path(path)` in `lib.rs` → `file_open_path`, move to
`commands/file_commands.rs`
