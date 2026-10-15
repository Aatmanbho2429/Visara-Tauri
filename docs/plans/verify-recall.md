# Verify recall at scale — Implementation Plan

> **Status:** proposed, not started.
>
> - [ ] Phase 0 — Instrumentation: log where verification stopped, so the problem shows up in a user's log
> - [ ] Phase A — Collapse byte-identical copies before verify (no re-index)
> - [ ] Phase D — Make the verify time budget real (no re-index)
> - [ ] Phase B — Separate, backfilled identity store (SIFT-VLAD), keyed by content hash
> - [ ] Phase C — Verify queue interleaves DINO order with identity order
> - [ ] Phase E — Calibrate on a real 60k library, then ship
>
> **Governing rule: no phase may make an existing library stale.** Nothing here
> touches `vectors.bin` or bumps `core::migrate::EMBED_SCHEMA_VERSION`. Every
> phase works on day one against what a user already has indexed. The new
> data from Phase B fills in gradually in the background. While it fills, search
> is never worse than it is today.

---

## The bug

A user with a ~60k-image library searches with a tile that *is* in the library. The
match never reaches the verified tier. A look-alike stone sits at #1 under "Similar patterns", and
the real match is somewhere below it, labelled `unchecked`.

You can reproduce it locally with the grey-marble query `2.webp` (a 696×1390 webp of a
grey marble slab with gold veining). The true match is `Image Db/BT JPG/BT-MG-6.jpg`.
SIFT/RANSAC proves it with 196 inliers and an inlier ratio of 0.676. The app never gets
to check it.

## Facts established before writing this (do not re-derive)

All measured 2026-10-15 on this Mac against the running sidecar.

**1. Verification reaches about 100 candidates, whatever the library size.** Real log line,
3907-file library:

```
[timing] search TOTAL ... candidates_indexed=3907 near_family_n=1965 verify_pool_n=400
verify_attempted_n=100 verify_budget_exhausted=true ... verified_n=0 ... verify_ms=13652.15
```

`verify_pass` checks `VERIFY_TIME_BUDGET_MS` (2.5 s) only *between* chunks of
`VERIFY_CHUNK` (100). One chunk takes about 12 s, so in practice the "2.5 s budget" lasts
about 12 s and covers exactly one chunk. `VERIFY_MAX_CANDIDATES` (400) is never reached.

**2. The near family is about 50% of the library.** `[calibration] ... pct_at_floor=50.3%`. At
60k that is roughly 30k candidates in the family, and the first 100 of them get checked.

**3. The local test library is mostly copies.** `meta.db` has 3907 `files` rows but only 296
distinct `hash` values; some files are copied 24×. Every copy has its own vector and
its own slot in the verify queue.

**4. Where the true match lands.** Ranks are among the 296 distinct images. "Queue slot" is
the match's position in the app's real verify queue, with copies counted.

| query | DINO rank | queue slot | checked? |
|---|---|---|---|
| `1.webp` | 8 | 100 | yes, by one slot |
| `2.webp` | 14 | 198 | **no** |

Locally, the copies are what push it out. On a real 60k library the same thing
happens without any copies: if 13 of 296 distinct images out-score the match on DINO
cosine, about 2,600 of 60k will. DINO's global embedding ranks how much a tile
*looks like* the query (grey marble), not whether it is *the same* slab. The bigger
the library, the more look-alikes there are.

**5. The descriptors already stored cannot fix the ordering.** Rank of `BT-MG-6.jpg` under each
descriptor already stored in `vectors.bin`:

| query | DINO embed | Gram | rose | 0.4 rose + 0.6 Gram | embed + Gram |
|---|---|---|---|---|---|
| `1.webp` | **8** | 164 | 62 | 122 | 51 |
| `2.webp` | **14** | 173 | 50 | 148 | 68 |

DINO is already the best of what is stored, so the fix needs a new descriptor. That is why Phase B
exists. It is also why Phase B has to be added alongside the existing data, not replace it.

**6. An instance-level SIFT signature does rank it first.** RootSIFT (same SIFT settings as
`pipeline.verify_one`: `nfeatures=1500`, `contrastThreshold=0.01`, `edgeThreshold=20`, grayscale,
max side 1024), aggregated into VLAD with a K=64 codebook (8192-d) and intra-normalisation plus
power-normalisation:

| query → match | DINO | VLAD K=64 | VLAD K=16 | interleave slot |
|---|---|---|---|---|
| `1.webp` → `BT-MG-6.jpg` | 8 | **1** (0.319 vs best other 0.240) | 10 | 2 |
| `2.webp` → `BT-MG-6.jpg` | 14 | **1** (0.378 vs 0.222) | 1 | 2 |
| yellow → `image_part.png` (recolour) | 1 | 1 | 5 | 1 |
| yellow → `Screenshot … 2.09.52 PM.png` (recolour) | 3 | 28 | 69 | 5 |
| yellow → `big image.png` (found inside) | 60 | 126 | 120 | 119 |

"Yellow" is the canonical regression query `Screenshot 2026-08-06 at 2.12.12 PM.png`.
Its filename contains U+202F, so resolve it by glob.

What this table shows:

- VLAD finds the *same slab* where DINO can't. DINO finds the *recoloured* design where
  VLAD can't. Neither replaces the other, which is why Phase C interleaves them rather than
  re-ranking by one score.
- K=16 is not enough. The codebook needs K=64 or larger.
- `big image.png` (found inside a larger slab) ranks poorly under **both**. This plan does
  not fix it. See "Out of scope".
- Caveat: the K=64 codebook in this probe was trained on the same 296 images it was tested on,
  which is optimistic. Phase B ships a codebook trained on a separate corpus, and Phase E
  re-measures.

**7. Cost.** SIFT on the 296 distinct images took 13.6 s on 8 threads, about 46 ms per file
of wall time. A 60k library of distinct images is about 45 min of background work, and less
with copies, since the store is keyed by hash.

**8. `files.hash`** is `SHA-256(file_size || first 64 KiB)` (`utils::file_utils::fast_hash`),
indexed by `idx_hash`. It already identifies byte-identical copies, and the sync pipeline
already trusts it to detect moves.

---

## Phase 0 — Instrumentation

Add to `[timing] search TOTAL`:

- [ ] `distinct_family_n`: near-family size after collapsing by hash (Phase A).
- [ ] `verify_reached_cos`: the cosine of the last candidate actually checked. If it is well
      above `NEAR_FAMILY_MIN_SIM`, the verify stage is what bounds recall, not the floor.
- [ ] `identity_coverage`: the fraction of the near family that has an identity signature
      (0 until Phase B).

Ship this first, even on its own. It is how we confirm on a user's machine that this bug is the
one they are hitting, without guessing.

## Phase A — Collapse byte-identical copies before verify

Files: `services/search/search_service.rs`, `core/database.rs`.

- [ ] Load `hash` alongside `(id, path)` in the scope id map (`database::folder_id_map`, or a
      sibling function that returns `(id, path, hash)`).
- [ ] Build the verify queue from **one representative per hash**: the highest-cosine copy that
      still exists on disk.
- [ ] After each chunk, copy the representative's verdict to every copy. All copies keep their own
      `SearchResult` row and still appear in results. They just don't each spend a verify slot.
- [ ] Keep the "same image as the query" exclusion (`path == query_abs`), and also exclude other
      copies of the query's own file when its hash is known.
- [ ] Progress `total` counts representatives, so the progress bar means "distinct images checked".

Effect on the local repro: the `2.webp` match moves from queue slot 198 to slot 14, so it gets
checked. No re-index; no new data.

Risk: two different files with the same size and the same first 64 KiB would share a verdict. For
compressed JPEG/PNG that does not happen in practice. If it ever matters, hash the full file for the
representatives only.

## Phase D — Make the time budget real

Files: `services/search/search_service.rs`, `config.rs`.

- [ ] Stop letting one 100-candidate chunk run the whole budget. Either start with smaller
      chunks (e.g. 20, then grow), or have the budget check cut a chunk short. Keep Phase 2b's
      reason for chunks of 100 in mind: the sidecar re-SIFTs the query once per request. If chunks
      get smaller, cache the prepared query in the sidecar for the search's duration instead of
      reverting.
- [ ] Re-decide `VERIFY_TIME_BUDGET_MS` from real `verify_ms` numbers once the budget is enforced.
      It is currently a value that is never actually applied.

This phase does not improve recall by itself, because about 100 checks is about what the
hardware gets through. What it does is make the latency honest, so Phase C's ordering is what
decides recall.

## Phase B — Separate, backfilled identity store

The core of the fix. It adds a second descriptor **next to** `vectors.bin`, never inside it.

**Storage.** A new `~/.pictoria/identity.bin` (path in `config.rs`) holds one signature per
**content hash**, not per `faiss_id`:

- Copies share one signature, so backfill work scales with the number of distinct images.
- The signature survives file moves and renames.
- `vectors.bin` and its format are untouched.
- The file has its own header with `IDENTITY_SCHEMA_VERSION` and a codebook id. Rust owns every
  write, as with the other stores.

**Versioning, independent of `EMBED_SCHEMA_VERSION`.** If the identity version or codebook
changes, only `identity.bin` is discarded and backfilled again; DINO vectors are never touched.
Search keeps working throughout, falling back to DINO order for any hash without a signature.
Add a comment block above `IDENTITY_SCHEMA_VERSION` as its changelog, mirroring the one
above `EMBED_SCHEMA_VERSION`.

**Descriptor.**

- [ ] In the sidecar, compute RootSIFT with the same settings as `verify_one`, then VLAD with K=64,
      intra-normalisation, power-normalisation and L2.
- [ ] Compress the 8192-d vector with PCA-whitening to about 1024-d, stored as f16 (about 2 KB per
      distinct image, about 120 MB at 60k). Fall back to f16 8192-d (16 KB per image, about 1 GB at
      60k) only if PCA measurably hurts the Phase E numbers.
- [ ] Ship the codebook and PCA matrix as a frozen sidecar asset, trained **offline once** on a broad
      tile corpus that is not any one user's library. The sidecar reports the codebook id through
      `/health`. Rust refuses a mismatch with `identity.bin`'s header, the same way `EMBED_DIM` is
      checked today.

**Sidecar API** (update `.claude/rules/sidecar.md`):

- [ ] Add an `identity` field to `/describe` results for newly indexed files. This needs its own
      decode at 1024, since `describe_batch` decodes at 512.
- [ ] Have the search-priority `/describe` also return the query's identity signature, so search
      doesn't need a second round trip.

**Backfill** (follow `services::tags::backfill_colors`):

- [ ] List the hashes in scope that have no signature at the current identity version.
- [ ] Process them in chunks on the `index` priority lane. Yield to searches via
      `search_gate::wait_while_active()`, as the colour backfill does.
- [ ] Save `identity.bin` periodically, so a quit loses at most one chunk. Resume by recomputing the
      set of missing hashes; no marker file is needed.
- [ ] Spawn it after `library_sync_complete`, next to the colour backfill. Emit progress so the UI
      can show something like "Improving search accuracy… 41%" without blocking anything.
- [ ] When a hash disappears from `files`, drop its signature during the store's compaction.

## Phase C — Interleaved verify queue

File: `services/search/search_service.rs`.

- [ ] Stage 1 stays the same: the DINO near family, at the `NEAR_FAMILY_MIN_SIM` floor.
- [ ] Score each representative from Phase A that has a signature by identity similarity
      (a dot product against the query's signature).
- [ ] Build the verify queue by alternating picks: next best by DINO, then next best by identity,
      skipping anything already queued. Representatives without a signature appear only in the
      DINO sequence.
- [ ] The final display order stays as it is today: verified first, then by `similarity`. Identity
      decides only what gets checked, never what the UI shows as "% pattern match".

Two guarantees to keep, with a unit test for each:

1. A candidate's slot is at most 2× its DINO rank. Search never reaches less than half as far
   down the DINO order as it does today, and at 0% coverage the queue is exactly today's.
2. The identity #1 is always in slot 2.

Measured effect: both marble queries go from slots 100 and 198 to slot 2.

## Phase E — Calibrate, then ship

- [ ] Run `/test-app`. The yellow regression case must still verify all three matches and still
      reject `2.09.52 PM → BRASIL GREY P4.jpg`. All 21/21 `NATURAL EMPIRE_*` colourway pairs must
      still verify.
- [ ] Add `1.webp` and `2.webp` → `BT-MG-6.jpg` as regression cases. Both must verify on the local
      library *with* its 24× copies present.
- [ ] Get a real `[timing] search TOTAL` line from a 60k-library user, before and after. Quote the
      lines; don't estimate. Phase 0's `verify_reached_cos` and `identity_coverage` are what show
      whether the change worked.
- [ ] Re-measure the VLAD margin with the offline-trained codebook. The local margins (0.38 vs 0.22)
      came from a codebook trained on the test set itself.
- [ ] Turn `CALIBRATION_LOGGING_ENABLED` off once `NEAR_FAMILY_MIN_SIM` has real data behind it
      (separate item; see `search-pipeline.md`).

---

## Rollout order

Phases 0, A and D are small, need no new data, and can ship in one release. They fix the
copied-folder case outright, since that is exactly what the local repro is.

Phases B and C ship together in a later release. Users update, search works immediately exactly as
before, and it gets better as the background backfill fills `identity.bin`. Nobody sees a
"re-indexing your library" screen, and no stored vector is ever invalidated.

## Out of scope

- **Found inside a larger slab** (`big image.png`): DINO ranks it 60 and VLAD 126. It needs a
  region-level signal: e.g. VLAD over tiles of the candidate, or matching the query's VLAD
  against sub-windows. That deserves its own plan; this one must not regress it (Phase E).
- **Lowering `NEAR_FAMILY_MIN_SIM`**: in the probes above the floor never excluded a true match.
  The cut that lost matches was the verify prefix, not the floor.
- **Porting SIFT to Rust**: rejected before; see the sidecar decision on record.

## Critical files

- `UI/src-tauri/src/services/search/search_service.rs`: `verify_pass`, `execute`, queue building
- `UI/src-tauri/src/core/vector_store.rs`: pattern to follow for the new `identity_store.rs`
- `UI/src-tauri/src/core/database.rs`: `folder_id_map` plus hash, list of missing hashes
- `UI/src-tauri/src/services/tags/tags_service.rs`: `backfill_colors`, the backfill template
- `UI/src-tauri/src/core/watcher.rs`: spawns the backfill after sync
- `UI/src-tauri/src/config.rs`: new path, `IDENTITY_SCHEMA_VERSION`, budget constants
- `sidecar/pipeline.py`, `sidecar/server.py`: identity descriptor, `/describe` field, `/health` codebook id
