---
paths:
  - "UI/src-tauri/src/services/search/**/*.rs"
  - "UI/src-tauri/src/services/match_engine/**/*.rs"
  - "UI/src-tauri/src/core/vector_store.rs"
  - "UI/src-tauri/src/core/feature_store.rs"
  - "UI/src-tauri/src/core/match_index.rs"
  - "UI/src-tauri/src/core/search_gate.rs"
---

# Search pipeline (`services::search`)

1. Describe the query image via the sidecar (`/describe`).
2. Select the **near family**: every indexed file whose DINO embedding cosine similarity is
   `>= config::NEAR_FAMILY_MIN_SIM`. A floor, not a top-N rank cut — the old fixed 1500-item
   shortlist could silently exclude a real match in a large library. Computed in-process against
   `VectorStore`.
3. Geometrically verify the *entire* near family via the sidecar's `/verify` (SIFT/RANSAC). This
   backs the `verified`/`partial`/`mirrored` flags and the match-point count in `SearchResult`.
   Nothing is truncated here either.
4. Secondary Gabor-rose + Gram-matrix score (`ROSE_WEIGHT`/`GRAM_WEIGHT`) is still computed and
   returned, but no longer selects candidates — it only breaks ties at equal embedding cosine.

5. **Deep tier**: when the request's `deepMinSim` is below
   `NEAR_FAMILY_MIN_SIM`, stage 1 runs once at that lower floor and the band below 0.70 is
   SIFT-verified after the fast tier. Only deep-tier rows that verify are returned; it is capped by
   `DEEP_VERIFY_MAX_CANDIDATES` / `DEEP_VERIFY_TIME_BUDGET_MS` and stops on `search_cancel`.

6. **Full DB**: the request's `fullDb` drops the stage-1 floor,
   both candidate caps and both time budgets (`VerifyLimits`), so every file in scope is verified;
   only `search_cancel` bounds it. Tier A and the verified-only deep rows behave as in step 5.

## Exact-match engine (`config::MATCH_ENGINE_MODE`, env `PICTORIA_MATCH_ENGINE=off|shadow|primary`)

A local TinEye-style layer that finds exact, resized, rotated, mirrored, cropped and recoloured copies
independently of DINO. Design and constants: `docs/plans/exact-match-engine.md`.

- **Stage 0 (pHash):** each file has a 64-bit DCT hash (`file_fingerprints`); the query's 8 dihedral variants are
  compared by Hamming distance. `<= EXACT_HASH_MAX_DIST` with colour cosine `>= EXACT_COLOR_MIN_SIM` is `exact`
  (`exactTransformed` when a rotated/mirrored variant won), same hash with other colour is `colourway`.
  Hits missing from stage 1 are scored with `VectorStore::score_ids` and join the first partial.
- **Stage 0b (retrieval):** the query's SIFT features (straight and mirrored) are quantised against the vocabulary and
  ranked over the inverted index (`core::match_index`); the top `MATCH_SHORTLIST` join the verify set. Skipped until a
  vocabulary exists. Shortlist-only rows appear only once they verify.
- **Verification:** candidates with stored features (`core::feature_store`) go through `/verify_features` with no
  decode; the rest fall back to `/verify`. Tier A / deep / Full DB limits are unchanged. `verified` with
  `MATCH_COPY_MIN_INLIERS` inliers and ratio `MATCH_COPY_MIN_RATIO` is a `copy`.
- **Order (backend `match_tier::compare_results` = UI `result-order.ts`):** exact, exactTransformed, colourway
  (hash distance, then similarity), copy (inliers), then the legacy order (verified, then similarity); path last.
- **Modes:** `off` is today's search (index-time extraction still runs); `shadow` (default) returns today's results with
  `matchKind: "none"` and, after `search_complete`, logs one `[match_engine] shadow ...` line comparing both
  verifiers; `primary` applies the above.

### Engine files (`~/.pictoria/`, written by Rust only)

- `features.bin`: header (magic `PICTFEAT`, `MATCH_FEATURES_VERSION`, descriptor width 128), then append-only records
  `id i64, w u16, h u16, n u16, pts f32 x 2n, desc u8 x 128n`, keyed by vector id. Load scans record headers only and seeks
  past payloads; descriptors are never resident and `read` is a positioned read. A replaced or removed record is tombstoned
  in place (`id = i64::MIN`); past 30% dead bytes the file is rewritten. A version mismatch deletes all three files.
- `words.bin`: `id i64, n u16, words u32 x n`, fully loaded, rewritten atomically on `save` (`u32::MAX` = unassigned).
- `vocab.bin`: header plus `k x 128` f32 centroids, trained once from `MATCH_VOCAB_SAMPLE` descriptors via `/train_vocab`.
- Vector ids are reused (`MAX(faiss_id) + 1`), so every removal path also calls `match_engine::remove_ids`, and indexing a
  file with no features drops any stale record for its id.

The `MATCH_*`, `EXACT_*` constants are uncalibrated starting points (tune with `sidecar/tools/eval_exact_match.py`).
`[timing] search TOTAL` also carries `engine_mode`, `exact_n`, `colourway_n`, `copy_n`, `engine_shortlist_n`,
`engine_retrieve_ms`, `query_features_ms`, `feature_verify_n`, `fallback_verify_n`, `vocab_ready`, and `[rank] top5`
lists the first five rows.

## Watch the floor

`NEAR_FAMILY_MIN_SIM` (0.70) is the only bound on how much work a search does, and it is not
calibrated against the current model. The number to watch is `near_family_n` in the
`[timing] search TOTAL` log line — if it's a large fraction of the library on an ordinary query,
the floor is too low and every downstream stage pays for it.

Don't hard-code a replacement threshold from local results. The only library available locally is
~400 images; calibration needs real data.

## Reporting timings

`[timing] search TOTAL` carries `near_family_n`, `verified_n`, `describe_ms`, `near_family_ms`,
`verify_ms`, `total_ms`. It's the only way to tell whether a slow search is the sidecar, the
near-family scan, or SIFT. Quote a real line from `Pictoria.log` — never an estimate, and never a
number from a debug build presented as a release cost.
