# Exact-match engine (TinEye-style) — development plan

Status: plan. Branch: `search-ranking-v2`. Supersedes parts of `search-ranking-v2.md` (see §10).

## 1. Goal

Replace the exact-match machinery with a local, TinEye-style engine that finds exact, resized,
re-saved, rotated, mirrored, cropped and lightly edited copies of the query in milliseconds,
**independently of DINO**. Keep the DINO "similar design" search exactly as it is.

Today, "exact" is SIFT/RANSAC run at search time on DINO's near family (`pipeline.verify_one`
via `/verify`). It has three faults:

- A copy DINO ranks low is never checked.
- Every check re-decodes the candidate and re-runs SIFT, about 35–125 ms each per `config.rs`.
- Verified rows bury an unverified true match.

## 2. Architecture

```
                  index time (per file)                        search time
sidecar  /extract_features ─► keypoints + SIFT desc ─┐   query ─► /extract_features (normal + mirrored)
sidecar  /describe (unchanged) + pHash (done)        │                 │
                                                     ▼                 ▼
Rust     feature_store (features.bin, words.bin) ◄─ quantize ─► match_index (inverted index, tf-idf)
Rust     vocab.bin  ◄── /train_vocab (sidecar k-means, once)          │ top MATCH_SHORTLIST ids
                                                                      ▼
sidecar  /verify_features (stored kp+desc, no decode) ─► existing RANSAC + geometry gate
                                                                      ▼
Rust     tiers: exact (pHash+colour) > colourway > copy (strong verify) > existing DINO ordering
```

The sidecar still never persists anything. Rust owns `features.bin`, `words.bin` and
`vocab.bin` under `~/.pictoria/`.

## 3. Part A — finish the pHash fingerprint (already on the branch)

Commits `566077f` (sidecar) and `8a2cc7c` (Rust, WIP, tests not run) implement
`search-ranking-v2.md` §3.1 up to and including the backfill.

- Run and fix their tests.
- Add a startup backfill for every watched folder if it is missing.
- Implement the search-side classification exactly as in v2 §3.1 ("Search"): variants, Hamming,
  `EXACT_HASH_MAX_DIST`, `EXACT_COLOR_MIN_SIM`, `VectorStore::score_ids`, and including those
  rows in the first partial.
- It is gated by the mode flag (§7).

## 4. Part B — sidecar (`sidecar/pipeline.py`, `sidecar/server.py`)

### 4.1 Shared verification core (refactor, behaviour-preserving)

- Extract the match + RANSAC + geometry-gate loop of `verify_one` into
  `_verify_matches(qg_shape, kp1_pts, des1, kp2_pts, des2, c_shape, mirror, ratio, ransac_thresh, min_inliers)`.
  `kp*_pts` are float32 `[n,2]` arrays.
- `verify_one` decodes, runs SIFT, then calls it.
- Prove parity with a test: on a synthetic textured image pair (seeded numpy noise plus shapes,
  one side a rotated crop), the result dict before and after the refactor is identical. Capture
  the "before" output by running the old function first.

### 4.2 `extract_features(img_rgb) -> (pts f32[n,2], sizes f32[n], angles f32[n], desc u8[n,128], (w,h))`

- Decode with `load_image_rgb(path, max_dim=MATCH_FEATURE_MAX_DIM)`, using 1024 to match
  `verify_one`'s candidate decode so `candidate_size`/quad semantics are unchanged.
- Grayscale. Run `cv2.SIFT_create(nfeatures=MATCH_FEATURES_PER_IMAGE, contrastThreshold=0.01, edgeThreshold=20)`.
- Descriptors are rounded to uint8 with `np.clip(np.rint(d), 0, 255)`.
- Keep the strongest `n` features by response.
- Python constants mirror Rust `config.rs`; comment the mirror, as `_GRAM_ZOOM_SCALES` does.

### 4.3 Endpoints

All payload arrays are base64 of little-endian raw bytes. Document each shape in one comment line.

- `POST /extract_features {paths, priority, mirror?}`:
  - Returns `{"results":[{"path","w","h","n","pts","sizes","angles","desc"} | {"path","error"}]}`.
  - `mirror: true` horizontally flips before extraction (query side only).
  - Runs over the existing `_VERIFY_WORKERS` thread pool.
  - Does not require the DINO model.
- `POST /train_vocab {desc, n, k, iters}`:
  - Runs k-means on `desc` (u8 `[n,128]` → float32).
  - Use `cv2.kmeans` with `KMEANS_PP_CENTERS` and `attempts=1`, or minibatch k-means in numpy if
    `cv2.kmeans` is too slow for 200k×128 at k=4096. Time it and pick.
  - Returns `{"centroids": b64 f32[k,128]}`.
  - Runs at `"index"` priority.
- `POST /verify_features {query_path, mirror, candidates:[{path,w,h,n,pts,desc}], priority}`:
  - Prepares the query once (`prepare_query`, unchanged: 800 px, `_new_sift()` 1500 features).
  - Calls `_verify_matches` per candidate on the pool.
  - Returns the same per-candidate dict shape as `/verify` (`matched`, `inliers`,
    `inlier_ratio`, `box`, `candidate_size`, `scale`, `mirrored`, `reason`, `path`).
  - With this, Rust's `VerifyResult` parsing is reused as-is.

Add `/extract_features`, `/train_vocab` and `/verify_features` to `.claude/rules/sidecar.md`.

## 5. Part C — Rust storage and index

New `config.rs` constants, one line each:

- `MATCH_FEATURES_PER_IMAGE: usize = 400`
- `MATCH_FEATURE_MAX_DIM: u32 = 1024`
- `MATCH_VOCAB_K: usize = 4096`
- `MATCH_VOCAB_SAMPLE: usize = 200_000`
- `MATCH_VOCAB_MIN_IMAGES: usize = 300`
- `MATCH_STOP_WORD_FRAC: f32 = 0.10`
- `MATCH_SHORTLIST: usize = 100`
- `MATCH_COPY_MIN_INLIERS: u32 = 25`
- `MATCH_COPY_MIN_RATIO: f32 = 0.5`
- `MATCH_FEATURES_VERSION: u32 = 1`
- Paths: `FEATURES_PATH`, `WORDS_PATH`, `VOCAB_PATH`
- `MATCH_ENGINE_MODE`: see §7

Comment block: all of these are uncalibrated starting points, tuned with the §8 harness.

### 5.1 `core/feature_store.rs`

Model it on `core/vector_store.rs`: tombstones, `maybe_compact`, an IO guard, a magic + version
header. If the file version ≠ `MATCH_FEATURES_VERSION`, delete the file and let the backfill
rebuild it. Never touch `vectors.bin`, and do **not** bump `EMBED_SCHEMA_VERSION`.

- `features.bin`:
  - Header.
  - Append-only records: `id i64, w u16, h u16, n u16, pts [n][2] f32, desc [n][128] u8`.
  - An offset table (`id → (offset, len)`) rebuilt on load by scanning record headers only:
    seek past the payloads, never read them.
  - Descriptors are **not** held in memory. `read(id)` uses a positioned read.
  - Upsert appends a new record and tombstones the old one.
  - Compaction rewrites the file when dead bytes exceed 30%.
- `words.bin`:
  - Header.
  - Records: `id i64, n u16, words [n] u32`.
  - Fully loaded, small: ~16 MB at 10k files.
  - `u32::MAX` means unassigned (no vocab yet).
- `vocab.bin`: header plus `k × 128 f32`.
- API:
  - `upsert(id, w, h, pts, desc)`
  - `remove(ids)`, called everywhere `store.remove` is called in `services/sync/sync_service.rs`
  - `read(id) -> Option<Features>`
  - `ids_missing_words()`
  - `set_words(id, &[u32])`
  - `sample_descriptors(max) -> Vec<u8>` (uniform over files)
  - `save`/`load`
- Tests: round-trip, upsert replaces, tombstone and compaction, offset reads after compaction,
  version mismatch drops the file.

### 5.2 `core/match_index.rs` (pure, no I/O)

- `quantize(desc: &[u8], vocab: &[f32]) -> Vec<u32>`: nearest centroid by L2, using rayon over
  descriptors. Precompute `||c||²` so the distance is `||c||² - 2·c·x`.
- `MatchIndex::build(words_by_id)`: postings `word → Vec<(id_idx u32, count u16)>`, plus `df`,
  `idf = ln(N/df)`, and per-document norms of the sqrt-tf × idf vector.
- `query(&self, query_words, top) -> Vec<(id, score)>`:
  - Count query words.
  - Skip words with `df > MATCH_STOP_WORD_FRAC × N`.
  - `score_d = Σ_w idf_w² · sqrt(q_w) · sqrt(c_dw) / (‖q‖ ‖d‖)`. Square-root tf is the
    burstiness damping for repeating tile textures.
  - Return the top `top` entries by a partial sort.
- A `resident()` cache like `vector_store::resident`, invalidated when words change.
- Tests:
  - quantize picks the nearest centroid.
  - A planted document sharing most words with the query ranks first among 1,000 random
    documents.
  - Stop words are ignored.
  - Burstiness: one word repeated 50× doesn't beat 20 distinct shared words.
  - Empty index and empty query behave sensibly.

### 5.3 Pipeline wiring

- **Index:** `services::sync::index_chunks` and `reembed_folder` call
  `sidecar::extract_features` for the chunk after `/describe`, then `feature_store.upsert`. If a
  vocab exists, they also quantize and `set_words`. A feature failure must never fail the file's
  indexing: log it and leave the file for the backfill.
- **Backfill** (`services::match_engine`, new folder, re-exported via `mod.rs`):
  - `backfill_features(folder)`: chunks of 64, waits on `search_gate`, logs one `[timing]` line.
    Spawn it like the fingerprint backfill, after sync and at startup.
  - `ensure_vocab()`: once the store holds ≥ `min(MATCH_VOCAB_MIN_IMAGES, total indexed files)`
    and there's no vocab, sample, train via `/train_vocab`, save `vocab.bin`, then assign words
    for every file missing them. Rayon, in a background thread, waiting on `search_gate`. Log
    `[timing] vocab train` and `[timing] word assign`.
- **`core::sidecar`:** typed client functions for the three new endpoints, base64 via the
  `base64` crate. Check `Cargo.toml`; add it if absent. Timeouts scale with n, like
  `verify_timeout`.

## 6. Part D — search integration (`services::search::execute`)

Primary-mode flow; §7 covers shadow mode:

1. **Stage 0, pHash:** exact, exactTransformed and colourway rows (Part A).
2. **Stage 0b, engine retrieval:**
   - Call `/extract_features` for the query twice, normal and `mirror: true`, in one call with
     two "paths" if simpler.
   - Quantize both and query `MatchIndex` for each.
   - Merge the results, keep each id's best score, and take the top `MATCH_SHORTLIST`.
   - Skip this stage when there's no vocab yet.
3. **Stage 1, DINO near family:** unchanged.
4. **Stage 2, verification:**
   - The verify set is the engine shortlist ∪ the existing `verify_paths`. The tier A / deep /
     Full-DB semantics of `verify_limits` are unchanged.
   - Candidates that have stored features go through `/verify_features` in chunks of
     `VERIFY_CHUNK`, straight then mirrored per the existing mirrored-retry rule. That is the
     same `verify_pass` structure, so add a `features: bool` path rather than duplicating it.
   - Candidates without features fall back to the existing `/verify`.
   - Engine-shortlist hits not in the near family get a `Match` via `VectorStore::score_ids`, so
     they can be displayed.
5. **Tiers and ordering** (backend final sort and UI `resortResults`, identical):
   1. `exact`
   2. `exactTransformed`
   3. `colourway`
   4. `copy`: verified with `inliers ≥ MATCH_COPY_MIN_INLIERS` and
      `inlier_ratio ≥ MATCH_COPY_MIN_RATIO`, ordered by inliers desc.
   5. Everything else in the **existing** order (verified first, then similarity).

   Within tiers 1–3, order by hash distance asc, then similarity desc. Final tie-break is `path`
   asc. `rank` is assigned on every partial, never 0.
6. **IPC:** `SearchResult` gets `match_kind: String`
   (`"exact"|"exactTransformed"|"colourway"|"copy"|"none"`) and `hash_distance: Option<u32>`.
   Mirror both in `UI/src/app/models/response/responseSearch.ts`. The current `VerifyResult`
   has no inlier ratio in `SearchResult`; keep it internal.
7. **Logging:** extend `[timing] search TOTAL` with `engine_mode`, `exact_n`, `colourway_n`,
   `copy_n`, `engine_shortlist_n`, `engine_retrieve_ms`, `query_features_ms`,
   `feature_verify_n`, `fallback_verify_n` and `vocab_ready`. Add `[rank] top5` (basename,
   matchKind, inliers, similarity).

## 7. Shadow mode (default) vs primary

- `config.rs`: `MATCH_ENGINE_MODE` defaults to `Shadow`. The env var `PICTORIA_MATCH_ENGINE`
  (`off|shadow|primary`) overrides it, so it can be tested without a rebuild. Parse it in one
  function in `config.rs`, with a unit test.
- **`Off`:** today's behaviour. No new work at search time; index-time extraction still runs so
  switching later is instant.
- **`Shadow`:** results, ordering and IPC values are exactly today's, with `match_kind` always
  `"none"`. After `execute` has built its results, it also runs stage 0, stage 0b and
  `/verify_features` on the engine shortlist plus the first 100 of `verify_paths`. It then logs
  one line:
  `[match_engine] shadow exact_n= copy_n= engine_only=[basenames…] legacy_only=[…] agree_n= feature_verify_ms= legacy_verify_ms=`.
  It runs after `search_complete` is emitted, so the user never waits for it.
- **`Primary`:** §6.

## 8. Part E — evaluation harness (`sidecar/tools/eval_exact_match.py`)

Standalone. Imports `pipeline` with `cv2`/`numpy` only. If `pipeline` imports torch or
onnxruntime at module level, make those imports lazy inside the functions that use them; that
is a small, behaviour-neutral change.

- `--library <folder> --n 200 --negatives 20 --seed 0 --out report.json`.
- For n random images, synthesise queries:
  - Resize 0.5×.
  - JPEG q=60.
  - rot90, rot180.
  - Mirror.
  - Centre crop at 50% and at 25% area.
  - Brightness/contrast ±20%.
  - Hue shift (a colourway proxy).
- For each query, run `extract_features` on the source and the negatives (random other library
  files), then `_verify_matches`.
- Report per transform:
  - True-positive rate: the source verifies.
  - False-copy rate: a negative passes the `copy` thresholds.
  - Median and p95 ms per candidate verify.
  - Mean feature bytes per image.
- Also compute pHash Hamming to source and to negatives per transform.
- `--detector sift` is the only implemented choice. Other values exit with a clear "not
  implemented" message. XFeat needs vendored weights plus model code, a follow-up.
- Document usage in `sidecar/README.md` (a short section).
- A smoke test runs it on 30 generated synthetic textures (seeded) under the scratchpad and
  checks the report shape and that rot90/resize TPR = 1.0 on them.

## 9. Part F — UI (`UI/src/app/views/search/`)

- **"Best matches" strip** above the existing sections: rows with `matchKind !== 'none'`,
  larger cards, one badge each:
  - "Exact copy" (exact)
  - "Rotated / mirrored copy" (exactTransformed, or copy with `mirrored`)
  - "Same design · other colour" (colourway)
  - "Copy · found inside" (copy with `partial`) or "Copy" (copy)

  Those rows are excluded from the family/similar sections below. Hidden when empty, which is
  always the case in shadow mode.
- Keep the existing family/similar sections and their order (the similar search is unchanged).
- Use `track item.path` everywhere instead of `item.rank`.
- Add a small `#rank` badge on cards.
- Make `resortResults()` match the backend tier order exactly. Factor it into a pure function in
  its own file with a spec.
- Add i18n keys to every file in `UI/src/assets/i18n/`, using the English text where there's no
  translation.

## 10. Relation to `search-ranking-v2.md`

- §3.1 (fingerprint) is completed by Part A here.
- §3.2 (fused score) and §3.3 (visible-window verify) are **deferred**: the user chose to keep
  the similar-search ordering as it is, and feature-based verification removes most of the
  speed problem that §3.3 targeted.
- §3.6 is reduced to Part F.
- Add a one-line status note at the top of `search-ranking-v2.md` saying so.

## 11. Docs

- `.claude/rules/search-pipeline.md`: stage 0 (pHash), stage 0b (engine retrieval),
  feature-based verification and its fallback, tiers, modes.
- `.claude/rules/sidecar.md`: the endpoints.
- `CLAUDE.md` cross-file invariants:
  - Constants mirrored between `config.rs` and `pipeline.py`.
  - `MATCH_FEATURES_VERSION` bump rule: any change to feature extraction parameters.
  - The engine files are Rust-written, like `vectors.bin`.

## 12. Acceptance

- `cargo test` (from `UI/src-tauri`) passes, including every new test named above.
- `npm ci && npm test -- --watch=false && npm run build` pass.
- `python -m py_compile` on all touched Python files passes.
- The verify refactor parity test and the harness smoke test pass.
- Shadow mode leaves search results byte-identical to `Off`: unit-test the tier sort with
  `match_kind = "none"` everywhere against the legacy comparator.
- Comment style per `.claude/rules/code-format.md`.
