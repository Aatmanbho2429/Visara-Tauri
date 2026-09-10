# Search latency plan — 3-second search at 50k images

Bring search wall-clock from **79 s over 393 images** to **~3 s over 50,000 images**, without
losing the accuracy SIFT/RANSAC verification provides.

Work the phases in order. Each one compiles and ships on its own, so the work can be paused
between phases without leaving the app broken.

---

## How to execute this document

**Rules for whoever implements this — read before touching code.**

1. **Do not skip Phase 0.** The measurements this plan is built on came from a *debug* build.
   Phase 0 establishes which Rust-side costs are real. Phase 5 is explicitly gated on it.
2. **Measure before and after every phase.** Capture the `[timing] search TOTAL` line from
   `%LOCALAPPDATA%\com.pictoria.app\logs\Pictoria.log`. Never report a phase complete on a
   projected or estimated number — every claim traces to a line from an actual run.
3. **One phase per commit / PR.** Do not batch phases.
4. **Do not hard-code a new similarity threshold.** The only library available locally has
   ~400 images. Phase 3 ships instrumentation and a calibration procedure instead; the number
   gets set later, on real data.
5. **Stop and report** if a phase's acceptance criteria are not met, rather than proceeding to
   the next phase. In particular, if Phase 2 changes cause the known-good verification set to
   regress, revert that step and say so.
6. Commands run from `UI/` unless stated otherwise. Rust commands can also run as
   `cargo <cmd> --manifest-path UI/src-tauri/Cargo.toml`.
7. **Prerequisite:** `cargo build` / `cargo check` will fail unless a frozen sidecar binary
   exists at `UI/src-tauri/binaries/pictoria-sidecar-<host-triple>.exe` — Tauri's `externalBin`
   build script validates it eagerly. See `sidecar/README.md`.

### Conventions (per `CLAUDE.md` and `.claude/rules/`)

- One-line `//` comments above non-obvious functions and blocks. **No** `///` doc comments,
  no `//!`, no JSDoc, no multi-line comments.
- Request/response models mirror field-for-field between `UI/src-tauri/src/models/` and
  `UI/src/app/models/`, both camelCase (`#[serde(rename_all = "camelCase")]` on every struct).
- Commands in `commands/` stay thin and delegate to `services/`; new commands register in the
  `invoke_handler![]` list in `lib.rs` (not `main.rs`).
- Angular reads command/event names from `UI/src/app/core/tauri/tauri-commands.const.ts` and
  `tauri-events.const.ts` — never string literals.
- `ZoneWrapperService` stays the only `@tauri-apps/api` import site.

---

## Why this work exists

Measured baseline from the user's own log (2026-09-09, 393 indexed files, i5-1135G7 4c/8t):

```
near_family_n=213 (54% of the library cleared the 0.70 cosine floor)  verified_n=9
describe_ms=1079  store_load_ms=138  id_map_ms=10  near_family_ms=60
verify_ms=77742   total_ms=79102
```

**Verification is 98% of the wall clock.** Two independent multipliers cause it:

1. **The near family is unbounded and the floor does nothing.** `NEAR_FAMILY_MIN_SIM = 0.70`
   admitted 54% of the library. At 50k that projects to ~27,000 candidates all sent to
   SIFT/RANSAC. `config.rs:28-34` already flags this constant as uncalibrated.
2. **Each candidate costs ~365 ms of wall clock even with the 8-way pool** — i.e. roughly no
   parallel speedup at all. `cv2.setNumThreads` is never called anywhere in the sidecar, so up
   to 8 `ThreadPoolExecutor` workers each drive OpenCV's full internal thread pool on 4
   physical cores.

### The caveat that shapes the phase order

Those measurements came from a **`target\debug` build** — the log shows the sidecar resolving
from `target\debug\`. `[profile.dev]` sets no `opt-level`, so it is 0, and every **Rust-side**
number (`store_load_ms`, `near_family_ms`, `id_map_ms`) is unoptimized and cannot be trusted.
The **Python-side** numbers (`describe_ms`, `verify_ms`) run in a separate process and are real
regardless.

### Scope

| | |
|---|---|
| **In** | Bounding verification work; cutting per-candidate verify cost; progressive result streaming; the Rust hot path (gated). |
| **Out** | Indexing throughput (`avg_describe_ms_per_file=1604`, ~22 h for 50k) and Browse grid virtualization. Both are real problems — separate work streams. |
| **Deferred** | Precomputed SIFT descriptors (Phase 6 — design recorded, do not build). |

---

## Phase 0 — Measure in release

Blocking prerequisite. The Rust-side cost profile is currently unknown.

1. `UI/src-tauri/Cargo.toml` sets `opt-level = "s"` on `[profile.release]` — optimize for
   *size*, which suppresses vectorization in the `near_family` numeric loop. Add a measurement
   profile; **do not** change the shipping profile yet:
   ```toml
   [profile.release-perf]
   inherits  = "release"
   opt-level = 3
   ```
2. Build and run a release build against the existing 393-image library. Run the same query as
   the baseline (`small_blue.png` against `D:\ImageDb`).
3. Record the `[timing] search TOTAL` line for release (`opt-level = "s"`) and for
   `release-perf` (`opt-level = 3`).

**Acceptance:** three comparable `[timing] search TOTAL` lines (debug / release / release-perf)
recorded in the PR description.

**Decision gate:** if release `store_load_ms + id_map_ms + near_family_ms` is under ~150 ms at
393 files (≈ under 1 s projected to 50k), **say so explicitly and move Phase 5 below Phase 4**.
Otherwise keep Phase 5 where it is.

---

## Phase 1 — Bound the verification work

This is what makes a 3-second answer possible at *any* library size. Do it before tuning
per-candidate cost.

### 1a. Verify in descending cosine order, against a budget

`VectorStore::near_family` (`core/vector_store.rs:321`) already returns matches sorted by
`embed_sim` descending, and `search_service.rs:204-213` preserves that order into `candidates`.
The verify pool is already correctly ordered — it just needs a bound.

In `UI/src-tauri/src/config.rs`:
```rust
// Verification stops when either bound is hit; whatever is proven by then is returned.
// The count cap is the safety net for the uncalibrated NEAR_FAMILY_MIN_SIM above; the time
// budget is what the user actually feels. Both are deliberately generous because Phase 4
// makes partial results visible, so overrunning is no longer a frozen screen.
pub const VERIFY_TIME_BUDGET_MS: u64  = 2_500;
pub const VERIFY_MAX_CANDIDATES: usize = 400;
```

In `UI/src-tauri/src/services/search/search_service.rs`:
- In `verify_pass` (`:62-85`), check elapsed time before each chunk and stop early. Return
  which paths were actually attempted alongside the results.
- Truncate `family_paths` at `VERIFY_MAX_CANDIDATES` before the pass, keeping the
  highest-cosine prefix.
- Add `verify_attempted_n` to the `[timing] search TOTAL` log line (`:320-333`).

### 1b. Unverified must not read as rejected

`SearchResult.verified: bool` currently conflates "SIFT proved no match" with "never checked".
Under a budget the second case becomes common and must be distinguishable.

- Add to `SearchResult` in `models/response/response_search.rs:15-41`:
  ```rust
  // "verified" = geometric proof; "rejected" = SIFT ran and refused; "unchecked" = the
  // budget ran out before this candidate was reached.
  pub verification: String,
  ```
  Keep `verified: bool` as-is so nothing downstream breaks — `verification` is additive.
- Mirror in `UI/src/app/models/response/responseSearch.ts:12-34` as
  `verification: 'verified' | 'rejected' | 'unchecked'`.
- In `search.ts`, the tier getters `familyResults` / `similarResults` (`:177-186`) stay keyed
  on `verified`. Add a third bucket for `unchecked` so the UI can label it "not yet checked"
  rather than implying rejection.

### 1c. Fix the mirrored-retry trigger

`search_service.rs:238` fires the mirrored pass when nothing verified — **including when
nothing verified because every chunk errored**, doubling the cost of an already-failing search.

Have `verify_pass` report chunk failures, and skip the mirrored pass when any chunk failed or
when the budget was exhausted (there is no spare budget for a second full traversal anyway).

### 1d. Release the store read guard before verify

`search_service.rs:160` takes `store_io_read_guard()` and holds it to the end of `execute()`
(~`:336`) — across the entire multi-second verify phase — which blocks the indexer's
`store.save()` at `watcher.rs:545` for the whole search.

Wrap the load + `near_family` call (`:160-182`) in an explicit block so the guard drops before
verification begins.

**Acceptance:** a search on the 393-image library completes in under ~4 s, `verified_n` is
still 9, and the log shows `verify_attempted_n` < `near_family_n`.

---

## Phase 2 — Cut per-candidate verification cost

~365 ms wall clock per candidate with an 8-way pool — near-zero parallel speedup. Apply in
order, **re-measuring after each step**. Stop when the Phase 1 budget comfortably covers a few
hundred candidates.

### 2a. Stop OpenCV thread oversubscription — do this first

Confirmed: a repo-wide grep for `setNumThreads`, `OPENCV_NUM_THREADS`, `OMP_NUM_THREADS`,
`torch.set_num_threads` and `intra_op_num_threads` returns **zero matches**. Up to 8 pool
workers each drive OpenCV's full internal pool on 4 physical cores.

In `sidecar/pipeline.py`, at module scope:
```python
# Each verify candidate already runs on its own pool thread (server._run), so OpenCV's own
# parallel_for_ pool would multiply 8 workers by n_cores and thrash. One thread per call.
cv2.setNumThreads(1)
```

Also set `intra_op_num_threads` on the ORT `SessionOptions` (`pipeline.py:175-176`). The DINO
forward runs on the single `_worker` thread and contends with Rust's rayon pool
(`config::NUM_WORKERS = 8`) — the same contention `core/search_gate.rs` exists to mitigate.

**This is expected to be the largest single win in the phase. Measure it alone, before 2b.**

### 2b. Stop re-preparing the query once per chunk

`prepare_query` (`pipeline.py:701`) decodes the query at `max_dim=800` and runs SIFT on it
**once per HTTP request**. With `VERIFY_CHUNK = 20`, a 213-candidate family paid for that 11
times — 22 with a mirrored pass.

Raise `VERIFY_CHUNK` (`search_service.rs:47`) to ~100.

Coupling to check: `verify_timeout` (`core/sidecar.rs:88-90`) is `30 + n` seconds, so chunk 100
gives a 130 s ceiling — fine. Progress granularity is preserved because Phase 4 streams results
per chunk anyway.

**Note:** there is no cancellation. When a Rust `.timeout()` fires, reqwest abandons the
connection but Python's `list(pool.map(...))` runs to completion, occupying the single worker
thread and delaying the next chunk. Larger chunks mean fewer timeout boundaries but a higher
cost when one is hit — another reason the Phase 1 budget matters.

### 2c. Reduce per-candidate SIFT and matching cost

In `pipeline.py::verify_one`:
- Candidate decode is `max_dim=1600` (`:761`) while the query is 800. SIFT cost scales with
  pixel count — try 1024.
- `_new_sift()` uses `nfeatures=4000, contrastThreshold=0.01` (`:261`), 4× looser than
  OpenCV's 0.04 default. `BFMatcher.knnMatch` is brute force, O(|des1|·|des2|·128), so keypoint
  count enters the dominant term **quadratically** — try `nfeatures=1500`.

Both change match sensitivity, so they are **not free**. Validate against the known-good set
before accepting — see the acceptance criteria below.

**Acceptance:** per-candidate wall clock drops from ~365 ms to a measured, recorded number;
the 21 known colourway pairs still verify; and the known false positive `BRASIL GREY P4.jpg`
stays rejected (measured margins documented at `pipeline.py:589-603` and `:796-809`).

---

## Phase 3 — Calibrate the near-family floor

Instrumentation now; the number gets set later, on real data.

1. In `search_service.rs`, after `near_family` returns, log the cosine distribution of the
   scanned population — counts at or above each of 0.60/0.65/…/0.95, plus
   `near_family_n / live_count` as a percentage. One `log::info!` line.
2. Log the cosine of the highest-ranked and lowest-ranked **verified** result. The lowest
   verified cosine across many real queries is the empirical floor.
3. **Do not change the 0.70 default in this phase.** `VERIFY_MAX_CANDIDATES` from Phase 1 is
   what protects against it being too loose in the meantime.

**Calibration procedure — hand this to the user, to run on a real library:**

> Run 10–20 searches whose correct answer you already know. For each, record `near_family_n`,
> `live_count`, and the lowest cosine among results that came back `verified`. Set
> `NEAR_FAMILY_MIN_SIM` a little below the **minimum** of those lowest-verified cosines.
> If `near_family_n` is still a large fraction of the library after that, the descriptor —
> not the threshold — is the problem.

---

## Phase 4 — Progressive result streaming

Makes the 3 seconds *perceptible*: the ranked list appears as soon as stage 1 finishes, and
verification badges fill in behind it.

**Nothing like this exists today.** `execute` returns once; `on_verify_progress` is
`FnMut(usize, usize, bool)` and cannot carry results; and `isSearching` / `hasResults` are
mutually exclusive template branches (`search.html:145` vs `:177`), so no surface can render
results mid-search.

### 4a. Rust

- **New event** `search_partial`, added to `tauri-events.const.ts` beside the existing
  `SEARCH_PROGRESS` / `SEARCH_COMPLETE` / `SEARCH_ERROR` (`:6-8`).
- **New response model** in `models/response/response_search.rs`, mirrored in
  `UI/src/app/models/response/responseSearch.ts`:
  ```rust
  // Partial search delivery: the stage-1 ranking first, then verification updates keyed by path.
  pub struct ResponseSearchPartial {
      pub results: Vec<SearchResult>,
      pub done:    usize,
      pub total:   usize,
      pub phase:   String,
  }
  ```
  **Reuse `SearchResult`** — do not introduce a parallel type.
- **Restructure `execute`** (`search_service.rs:90`): build the `SearchResult` set from
  `candidates` *before* verification (all `verification: "unchecked"`), emit it, then update
  entries in place as chunks return.
- **Change the callback** so it can carry results:
  ```rust
  // Carries newly-updated results alongside the counters so the command layer can stream
  // them; `updated` is empty for a pure progress tick.
  pub struct VerifyProgress<'a> {
      pub done:     usize,
      pub total:    usize,
      pub mirrored: bool,
      pub updated:  &'a [SearchResult],
  }
  ```
- In `commands/search_commands.rs:126-141`, emit `search_partial` from that callback and keep
  emitting `search_progress` for the bar. `search_complete` still fires at the end and stays
  authoritative.
- **Throttling:** one emit per chunk is right once `VERIFY_CHUNK` is ~100. If chunk size is
  later reduced, throttle to `config::PROGRESS_EMIT_INTERVAL_MS` (400 ms). Note there is no
  existing throttle helper — `watcher.rs:505-521` inlines its own ticker thread.

### 4b. Angular

- `search.service.ts`: listen for `search_partial`, surface it as a new `SearchEvent` type.
- `search.ts`: on `partial`, **merge by `path`** into `state.results` rather than replacing
  wholesale (`:367`). Preserve the `thumbnailUrl` / `imgError` / `matchBox` view-model fields
  already attached to each row.
- `search.html`: let the results grid render while searching. Move the progress strip
  (`:154-170`) above the grid instead of being an exclusive branch, and keep the scan-rings
  animation only until the first partial arrives.
- `similarShown` currently resets on `complete` (`search.ts:385`). Reset it on the **first
  partial** instead, so paging state is established when results first appear.

**Acceptance:** results are visible before verification finishes; badges upgrade in place;
`search_complete` produces no visible flicker or reordering jump.

---

## Phase 5 — Rust hot path

**Only proceed if Phase 0 showed these costs are material in a release build.**

### 5a. Resident vector store

There is no long-lived store. `VectorStore::load` runs at `search_service.rs:161` on **every
search**, re-reading and re-parsing all of `vectors.bin` through a per-`f32` `from_le_bytes`
loop (`core/vector_store.rs:171-182`). Entry stride is 25,448 bytes (4608 embed + 8 rose +
1728 gram + 16 color floats, plus an i64 id) → **~1.19 GB at 50k**.

Use an in-memory singleton, following the crate's existing convention (`static X: Lazy<…>`
from `once_cell::sync`, as at `core/sidecar.rs:43`, `core/watcher.rs:94`, `core/progress.rs:66`):

```rust
// Loaded once and reused across searches; invalidated when indexing rewrites the file.
static RESIDENT: Lazy<RwLock<Option<Arc<VectorStore>>>> = Lazy::new(|| RwLock::new(None));
```

- Search clones the `Arc` and drops the lock immediately — composes with the Phase 1d fix.
- **Invalidate after indexing.** `watcher.rs:546` is the only `save` site; clear `RESIDENT`
  there, inside the existing `store_io_write_guard()` scope, so the next search reloads.
- Log the resident size once on load so RSS is visible in the field.
- Cost: ~1.19 GB RSS at 50k, ~7.1 GB at 300k. Fine at 50k on a 16 GB machine; **not** at 300k.

**Alternatives, and why not now:**

| Option | Verdict |
|---|---|
| `memmap2` | Strictly better long-term — the format is already a flat fixed-stride record array, so it maps zero-copy with no parse and no RSS. Costs a new dependency. Adopt if 50k RSS proves a problem, or before targeting 300k. |
| fp16 embeddings | Halves the store, but changes `VectorStore::VERSION`, which makes `load` start fresh and arms a **full re-index** (~22 h at 50k). Not worth it here. |
| Split hot embedding from cold rose/gram/color | Also a format change and the same re-index cost, for a 27% saving. No. |

### 5b. Dot-product fast path for the embedding scan

`best_zoom_sim(query_embed, e, EMBED_DIM)` (`vector_store.rs:341`) is the only thing evaluated
for all N entries — rose/gram/color are computed only for entries that already passed the
floor. It calls `cos_sim` (`:407`), which recomputes **both** norms every call: 27,648 wasted
multiplies against 13,824 useful ones per entry, and the query's norms are recomputed for all
50k entries.

`pipeline.py::embed_descriptor` L2-normalises **each zoom level independently** before
returning (`:390-396`), so stored embeddings are already unit vectors per level.

- Add a `dot(a, b)` helper and a `best_zoom_dot`, used **only** for the embedding.
- **Guarantee the precondition** by normalizing each zoom level in `VectorStore::upsert`
  (`:262`) — cheap, once per file — rather than trusting the producer. Add a debug assertion
  on load.
- **Do not change the rose, gram, or color paths.** `gram_descriptor` does not normalise at
  all and `gabor_rose` normalises to sum = 1 (L1, not L2). They must keep full `cos_sim`.
- Add a unit test asserting `best_zoom_dot` and `best_zoom_sim` agree to ~1e-5 on unit inputs,
  alongside the existing tests at `vector_store.rs:432-576`.

### 5c. `folder_id_map` full table scan

`core/database.rs:368-380` uses `WHERE path LIKE 'prefix%'`. SQLite's default `LIKE` is
case-insensitive, so `idx_path` is **not** used and this is a full scan of `files` — once per
scope folder (`search_service.rs:167-171`).

Replace with an index-usable range predicate rather than setting the global
`PRAGMA case_sensitive_like`, which would change behaviour elsewhere:
```sql
SELECT faiss_id, path FROM files WHERE path >= ?1 AND path < ?2
```
with `?2` the prefix with its last character incremented.

---

## Phase 6 — Precomputed SIFT descriptors (design only — do not build)

The structural fix for verification cost: `verify_one` decodes each candidate and runs SIFT on
it **at query time, every time**, for descriptors that never change. Recorded here so it does
not have to be re-derived.

- **Storage.** `files` has no BLOB column and `vectors.bin` is strictly fixed-stride
  (`vector_store.rs:26-40`), so neither can carry a variable-size payload. This needs a new
  artifact — `~/.pictoria/sift/<faiss_id>.bin`, or a dedicated SQLite table with a BLOB column
  keyed on `files(id)`.
- **Size.** SIFT descriptors are 128 bytes/keypoint. At `nfeatures=4000` that is ~512 KB/image
  → 25 GB at 50k. Capping at ~800 keypoints gives ~100 KB/image → ~5 GB, read only for
  shortlisted candidates and never loaded wholesale.
- **Benefit.** Removes decode + SIFT from the query path, leaving BFMatcher + RANSAC —
  roughly 60-70% of current per-candidate cost eliminated.
- **Migration.** Requires bumping `EMBED_SCHEMA_VERSION` (currently `9`, `core/migrate.rs:64`)
  or an equivalent marker, forcing a one-time full re-index. Add to the changelog comment block
  above that constant, as the existing convention requires.
- **Decide after Phase 2.** If per-candidate cost lands low enough there, this may not be worth
  the schema bump.

---

## Risks and regressions to watch

- **Recall.** `NEAR_FAMILY_MIN_SIM` replaced a fixed top-1500 shortlist specifically so a real
  match could not be excluded by rank (`search_service.rs:30-39`). `VERIFY_MAX_CANDIDATES`
  reintroduces a rank cut on the *verification* stage. That is acceptable only because
  (a) unverified results are still returned and labelled `unchecked` rather than dropped, and
  (b) verification runs in descending cosine order. **Do not let it silently truncate
  `results`.**
- **Store locking.** Phases 1d and 5a both touch the guards. The existing order is
  `SYNC_LOCK` (`watcher.rs:470`) → `STORE_RW_LOCK.write()` (`watcher.rs:545`); search takes
  only `STORE_RW_LOCK.read()`. Do not introduce a new nesting.
- **Memory.** Phase 5a adds ~1.19 GB RSS at 50k.
- **Sidecar crashes.** A crash mid-verify surfaces as a transport error, and the chunk loop
  keeps issuing chunks against a dead process — `sidecar::is_ready()` is checked only once, at
  `search_service.rs:107`. Larger chunks waste more time this way; consider re-checking
  readiness between chunks.
- **Schema.** Nothing in Phases 0-5 changes how a descriptor is produced, so **no
  `EMBED_SCHEMA_VERSION` bump is required.** Confirm this before merging — if any change does
  alter descriptor production, bump `core/migrate.rs:64` and add to its changelog comment.

---

## Verification

**Per phase:** each phase states its own acceptance criteria. Record a `[timing] search TOTAL`
line before and after.

**Rust unit tests** — `cargo test --manifest-path UI/src-tauri/Cargo.toml`
- `vector_store` — existing tests (`vector_store.rs:432-576`) must still pass; add the
  dot-product equivalence test from 5b.
- `search_service` — existing tests (`search_service.rs:436-453`) must still pass; add a test
  that the budget stops verification early and marks the remainder `unchecked`.

**Angular tests** — `npm test -- --watch=false` from `UI/`
- `search.spec.ts` — cover merging a `search_partial` into existing results by path without
  losing the view-model fields.

**End-to-end, on the real library:**
1. `npx tauri dev` (or a release build for timing runs); log in so the DINO model loads.
2. Search `small_blue.png` against `D:\ImageDb`. Confirm results appear before verification
   finishes, `verified_n` is still 9, and total wall clock is materially below 79 s.
3. Confirm the known-good verification set still behaves — the 21 colourway pairs verify and
   `BRASIL GREY P4.jpg` stays rejected (`pipeline.py:589-603`).
4. Start a folder re-index, then search while it is running. Confirm the search preempts
   indexing and that the Phase 5a store invalidation does not deadlock against `watcher.rs:545`.

**Do not report a phase complete on projected numbers** — every claim traces to a `[timing]`
line from an actual run.
