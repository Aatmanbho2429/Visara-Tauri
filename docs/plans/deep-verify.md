# Deep verify — Implementation Plan

> **Status:** implemented (Phases 1–6); only the manual walkthrough in Phase 6 is left, and the user runs it.
>
> - [x] Phase 1 — Config: restore the fast floor, add deep-verify constants
> - [x] Phase 2 — Rust: two-tier stage 1, deep verify pass, one continuous progress counter
> - [x] Phase 3 — Rust: cancellation (`search_cancel` command, and a new search replaces a running one)
> - [x] Phase 4 — IPC models: `deepMinSim` on the request, mirrored Rust ↔ TS
> - [x] Phase 5 — Angular: "Verify down to" dropdown, Stop button, progress strip covering both tiers
> - [x] Phase 6 — Tests (manual walkthrough still to be run by the user)
>
> **Working rule:** stored data is not touched. Everything here is search-time only.
> `vectors.bin`, `meta.db`, `EMBED_SCHEMA_VERSION` and the sidecar are all left alone.

---

## Why

A real match can score well below the 0.70 DINO floor and still be a genuine, SIFT-provable
match. The catalogue sheet `BT JPG/BT-Z25017.jpg` is one: the blue labradorite query shows it
at **48%** in the app. It only verifies once the floor drops below 0.48, and the second-placement
retry in `sidecar/pipeline.py::verify_one` is already in.

Lowering `NEAR_FAMILY_MIN_SIM` globally to reach those makes **every** search slow. At 0.20 on a
3,907-file library one search took 291 s (real log line: `near_family_n=3786 ... verify_ms=128876.83
... mirror_ms=160575.31 total_ms=291280.81`).

So keep the fast search exactly as it is, and do the slow part afterwards, in the background, while
the user is already looking at results.

## The flow the user sees

1. The user picks an image and clicks Search. A **"Verify down to"** dropdown sits next to the
   scope picker. Options: `70%` (deep search off), `60%`, `50%`, `40%`. The default is **70%** on
   every search (deep tier off); the choice is not remembered.
2. **Tier A (unchanged behaviour):** the grid paints the near family (cosine ≥ 0.70) as it does
   today. SIFT verifies it top-down, and the verified tier fills in.
3. **Tier B (new):** when Tier A finishes, verification carries on into the band
   `[deepMinSim, 0.70)`, highest cosine first. Grid rows for Tier B appear **only when they
   verify**; unverified Tier B rows never reach the grid. Otherwise "Similar" would flood with tens
   of thousands of weak look-alikes.
4. **One progress strip covers both tiers.** `done/total` counts Tier A and Tier B together, and the
   label says which tier is running (see "Progress" below). A **Stop** button ends deep search and
   keeps everything verified so far.
5. Clicking **New search**, or searching again while deep search runs, stops the running search
   first. The user is never told "a search is already in progress" because of a background tier.

## Facts the implementer needs (verified in the code, do not re-derive)

- `services/search/search_service.rs::execute` does stage 1 (`store.near_family(.., NEAR_FAMILY_MIN_SIM)`,
  sorted by `embed_sim` descending), then `verify_pass` over a prefix capped by
  `VERIFY_MAX_CANDIDATES` / `VERIFY_TIME_BUDGET_MS`, then a conditional mirrored pass. It streams
  through `on_verify_progress(VerifyProgress { done, total, mirrored, updated })`.
- `commands/search_commands.rs::search_start` turns each `VerifyProgress` into `search_progress`
  (with `percent = done/total`) and `search_partial` events, then `search_complete`. A second
  `search_start` while `SEARCH_STATE.running` gets a 409 `search_error`. There is no cancel today.
- `verify_pass` checks its deadline only **between** `VERIFY_CHUNK` (100) chunks. One chunk is about
  3.5–13 s on an Apple M5. Cancellation will act at the same granularity.
- `search_gate::begin()` is held for the whole of `execute`. It pauses the colour-tag backfill.
- Angular: `views/search/search.ts::runSearch` subscribes to `SearchService.searchStream` and never
  unsubscribes. `mergeResults` merges by `path`; `resortResults` sorts verified-first, then by
  `similarity`. `isVerifying` drives the `.verify-strip` in `search.html` (lines ~196–206).
  `newSearch()` only calls `state.reset()`; the Rust search keeps running.
- Current `config.rs` values are test leftovers: `NEAR_FAMILY_MIN_SIM = 0.40`,
  `VERIFY_MAX_CANDIDATES = 5_000`, `VERIFY_TIME_BUDGET_MS = 300_000`.

## Phase 1 — Config (`UI/src-tauri/src/config.rs`)

- [x] `NEAR_FAMILY_MIN_SIM` back to `0.70`. It is now only the Tier A floor.
- [x] `VERIFY_MAX_CANDIDATES = 2_000`, `VERIFY_TIME_BUDGET_MS = 300_000` (Tier A, as the user set
      them before testing).
- [x] Add these, each with a one-line comment (see `.claude/rules/code-format.md`):
  - `DEEP_VERIFY_DEFAULT_MIN_SIM: f32 = 0.70` (changed from 0.40: default is deep-off)
  - `DEEP_VERIFY_MIN_SIM_FLOOR: f32 = 0.30`, the lowest value the request may ask for. The
    command clamps to it.
  - `DEEP_VERIFY_MAX_CANDIDATES: usize = 20_000`
  - `DEEP_VERIFY_TIME_BUDGET_MS: u64 = 1_800_000` (30 min hard stop)

## Phase 2 — Rust search (`services/search/search_service.rs`)

- [x] `execute` takes a new `deep_min_sim: f32`. If it is `>= NEAR_FAMILY_MIN_SIM`, behaviour is
      exactly today's (no Tier B).
- [x] Run stage 1 **once** at `min(deep_min_sim, NEAR_FAMILY_MIN_SIM)`, then split the sorted
      candidates into Tier A (`embed_sim >= NEAR_FAMILY_MIN_SIM`) and Tier B (the rest). Don't call
      `near_family` twice; it is a full scan.
- [x] Tier A: today's code path, unchanged, including the mirrored retry. The initial all-`unchecked`
      partial contains **Tier A rows only**.
- [x] Tier B: after Tier A (and its mirror pass, if any), run `verify_pass` (straight only, **no
      mirrored pass**, since that would double an already long run) over Tier B, capped by
      `DEEP_VERIFY_MAX_CANDIDATES` / `DEEP_VERIFY_TIME_BUDGET_MS`. In its chunk callback, emit
      **only the Tier B rows that verified**: build them with `build_result` and push them into the
      final `results`. Tier B rejects and unchecked rows are dropped, never returned.
- [x] Final result set: all Tier A rows plus verified Tier B rows. Then the existing verified-first
      sort and rank assignment.
- [x] Extend `[timing] search TOTAL` with `deep_min_sim`, `deep_pool_n`, `deep_attempted_n`,
      `deep_verified_n`, `deep_ms`, `deep_stopped` (`none` | `cancelled` | `budget` | `cap`).
      Every timing claim about this feature must quote that line.

### Progress (one counter for the whole search)

- [x] Extend `VerifyProgress` with `tier: &'static str` (`"near"`, `"mirrored"`, `"deep"`), and use
      it to replace the `mirrored: bool` field. Update the one caller.
- [x] `total` is fixed at the start as `tierA_pool + tierB_pool`. If the mirrored pass triggers,
      add `tierA_pool` to `total` at that moment.
- [x] `done` is cumulative across all passes and never goes backwards.
- [x] Make `percent` monotonic in `search_start`: keep the last emitted value and emit
      `max(last, done/total*100)`, so a growing `total` never moves the bar backwards.
- [x] Phase labels, built in `search_start`:
  - `"Verifying matches"` (near)
  - `"Checking mirrored orientation"` (mirrored, as today)
  - `"Deep search · down to 40%"` (deep, with the real `deep_min_sim`)

## Phase 3 — Cancellation (`commands/search_commands.rs`, `search_service.rs`)

- [x] Add a `static SEARCH_CANCEL: AtomicBool`. `verify_pass` checks it before each chunk, as it
      checks the deadline. When set, it stops and returns what it has. Mark the result `cancelled`
      so the timing log says so.
- [x] New command `search_cancel` (thin; register it in `lib.rs`'s `invoke_handler`, and add
      `SEARCH_CANCEL` to `tauri-commands.const.ts`). It sets the flag and emits
      `search_cancel_response` per `.claude/rules/tauri-ipc.md`. A cancelled search still ends with
      a normal `search_complete` carrying everything verified so far.
- [x] `search_start` while a search is running: **don't** emit the 409. Set `SEARCH_CANCEL`, wait
      (the existing 150 ms poll) until `running` is false, clear the flag, then start. Expect a wait
      of up to one chunk (≤ ~13 s on an M5). While waiting, emit one `search_progress` with phase
      `"Stopping previous search…"` so the user isn't staring at nothing.
- [x] Clear `SEARCH_CANCEL` at the start of every search, so a stale flag can't kill the next one.

## Phase 4 — IPC models

- [x] `models/request/request_search.rs::RequestStartSearch` gains `deep_min_sim: Option<f32>`
      (camelCase `deepMinSim`). `search_start` takes `deep_min_sim: Option<f32>` and clamps it to
      `[DEEP_VERIFY_MIN_SIM_FLOOR, NEAR_FAMILY_MIN_SIM]`; `None` means
      `DEEP_VERIFY_DEFAULT_MIN_SIM`.
- [x] Mirror the field in `UI/src/app/models/request/requestSearch.ts` (see `.claude/rules/models.md`).
- [x] No response model change is needed. `search_progress` already carries `phase`/`done`/`total`/`percent`.

## Phase 5 — Angular (`views/search/`, `services/search/`)

- [x] `SearchService.searchStream(imagePath, scopePaths, topK, deepMinSim)` passes `deepMinSim`.
      Add `SearchService.cancel()` calling `search_cancel` through `ZoneWrapperService`
      (never `@tauri-apps/api` directly; see `.claude/rules/zone-wrapper.md`).
- [x] **Dropdown** in the `.scope-row` next to "Searching in", styled like the existing
      `scope-pill`/`scope-dropdown` (the custom dropdown already there, not a new PrimeNG
      component). Label: "Verify down to". Options: `70% (fast)`, `60%`, `50%`, `40% (deep)`.
      Store the value as a number (`0.7`, `0.6`, `0.5`, `0.4`) in `SearchStateService`. It always
      starts at 0.7 (not persisted, reset on New search). Disable it while a search is running. Add the strings to `assets/i18n/en.json`.
- [x] `runSearch` keeps the subscription in a field and unsubscribes it in `newSearch()` and at the
      start of the next `runSearch`, so an old search's late events can't write into a new search.
      `newSearch()` also calls `searchSvc.cancel()` when `state.progress.active`.
- [x] **Stop button** in the `.verify-strip`, shown while `isVerifying`. It calls
      `searchSvc.cancel()` and sets the label to "Stopping…". The final `search_complete` ends it
      as usual.
- [x] The strip already binds `phase`, `percent` and `done/total`, so it follows the Rust numbers
      without new logic. Check it stays visible across the Tier A → Tier B handover (the
      `active` flag must stay true until `search_complete`).
- [x] A verified Tier B row arrives through the normal `search_partial` merge and lands in the
      Verified section. No new section; its similarity (e.g. 48%) is shown as for any other row.

## Phase 6 — Tests and walkthrough

- [x] `cargo test`: unit tests for (a) the Tier A / Tier B split at a given `deep_min_sim`,
      (b) `deep_min_sim >= 0.70` producing no Tier B, (c) `verify_pass` returning early with nothing
      attempted when `SEARCH_CANCEL` is set before it starts (mirrors the existing budget test),
      (d) clamping of out-of-range `deepMinSim`.
- [x] `npm test -- --watch=false`: the existing search spec still passes. Add specs for the dropdown
      default/persistence, and for `newSearch()` calling `cancel()` while a search is active.
- [x] `cargo check`, `npm run build` clean.
- [ ] Manual (the user runs this; it needs the running app and the staged sidecar):
  - Blue labradorite query at 40%: `BT-Z25017.jpg` appears in Verified during the deep tier.
  - At 70%: identical behaviour and timing to before this change.
  - Stop during the deep tier: the strip ends, verified rows stay.
  - New search during the deep tier: starts after a short "Stopping previous search…".
  - Quote the `[timing] search TOTAL` lines for each.

## Cost the user should expect

The deep tier verifies at the same 35–125 ms per candidate (Apple M5) as Tier A. On the
3,907-file test library, the 0.40–0.70 band is roughly a third of the library. At 60k that can be
tens of thousands of candidates, so the deep tier can run for **many minutes**, bounded by
`DEEP_VERIFY_MAX_CANDIDATES` and the 30-minute budget. That is why it runs behind visible results
and has a Stop button. Duplicate files are not collapsed (the user's decision), so copies each
cost a check.

## Out of scope

- Collapsing duplicate copies, and the SIFT-VLAD identity store: see `docs/plans/verify-recall.md`.
- Mirrored retry over Tier B.
- Releasing `search_gate` during the deep tier. Background colour backfill stays paused until the
  search ends; revisit if that becomes a complaint.

## Critical files

- `UI/src-tauri/src/config.rs`
- `UI/src-tauri/src/services/search/search_service.rs`
- `UI/src-tauri/src/commands/search_commands.rs`, `UI/src-tauri/src/lib.rs`
- `UI/src-tauri/src/models/request/request_search.rs`, `UI/src/app/models/request/requestSearch.ts`
- `UI/src/app/services/search/search.service.ts`, `search-state.service.ts`
- `UI/src/app/views/search/search.ts`, `search.html`, `search.scss`
- `UI/src/app/core/tauri/tauri-commands.const.ts`, `UI/src/assets/i18n/en.json`
