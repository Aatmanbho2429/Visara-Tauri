# Search ranking v2 — exact-match tier, fused ranking, unified result view

Status: plan. Branch: `search-ranking-v2` (off `Iris_Creation_Dev`).

## 1. The problem

In libraries of about 10k images, the exact or near-exact copy of the query image (and its
close siblings) is not near the top of the results. Users watch it slide down the grid while
verification streams in.

## 2. Root causes (from the code, not from field logs)

1. **Verified rows always sort above every other row.** `search_service.rs::execute` sorts with
   `b.verified.cmp(&a.verified)` first, and `search.ts::resortResults` does the same after every
   streamed partial. The UI also renders two separate sections ("family" = verified, then
   "similar"). If SIFT/RANSAC rejects the true match, every verified row ranks above it, even a
   false positive at cosine 0.71 with 12 inliers. Two things make this likely:
   - Tile textures are repetitive and self-similar. Lowe's ratio test throws out ambiguous
     matches on repeating motifs, and plain or low-texture tiles return `too-few-keypoints`.
     Even a true duplicate can be "rejected".
   - False-positive "verified" rows grow linearly with library size: a fixed false-positive rate
     times thousands of candidates. At 10k images there are enough of them to bury the true match.
   - Each partial re-sorts the grid, so the rejected or unchecked true match visibly "moves down"
     as more rows verify. This is exactly what users report.
2. **Verified rows are ordered by cosine, not by geometric evidence.** 300 inliers and 12 inliers
   count the same.
3. **No exact-duplicate signal.** Retrieval is grayscale DINO only, using the best of 9 zoom-pair
   dot products. Colourways, siblings in the same series and the exact file all score about
   0.95–0.99, and colour carries zero ranking weight. Nothing says "this is the same image".
4. **Speed.** The fast tier verifies up to `VERIFY_MAX_CANDIDATES` (2000) with a 300 s budget.
   `config.rs` puts it at "~70-250 s". Rows far below anything the user will look at are
   SIFT-checked before the search settles.
5. **UI bug.** `@for (item of col; track item.rank)`: `rank` is 0 for every streamed partial
   (set only at the end), so Angular tracks every card by the same key while streaming.

## 3. Target design (Phase 1 + 2, implemented on this branch)

### 3.1 Exact / near-duplicate fingerprint tier (new)

A 64-bit DCT perceptual hash (pHash) per file. Comparing it is a Hamming distance:
`(a ^ b).count_ones()`. For 10k files that takes microseconds. The query side gets 8 dihedral
variants (4 rotations × optional mirror), so rotated and mirrored copies are also found.

**Sidecar (`sidecar/pipeline.py`, `sidecar/server.py`)**

- `fingerprint(img_rgb) -> list[str]`:
  1. Grayscale.
  2. `cv2.resize` to 32×32 with `INTER_AREA`, as float32.
  3. If the std of that 32×32 image is below `_FLAT_STD = 3.0` (0–255 scale), return `[]`.
     Flat or plain images get no fingerprint; otherwise they would all collide.
  4. Otherwise build 8 variants of the 32×32 array, in this order: `rot0, rot90, rot180, rot270,
     fliplr, fliplr+rot90, fliplr+rot180, fliplr+rot270` (`np.rot90`, `np.fliplr`).
  5. For each variant, run `cv2.dct` and take the top-left 8×8 block. Set bit i to
     `coef[i] > median(coefs excluding [0,0])`, packing row-major with bit 63 first.
  6. Format each hash as a 16-char lowercase hex string. Hex, not a JSON number: u64 values
     above 2^53 lose precision as JSON numbers.
- `_prepare` already decodes the image, so call `fingerprint(img)` there. Add
  `phash: list` to `Descriptor` and to the `describe` response dict in `server._run`.
- New job kind `"fingerprint"` and route `POST /fingerprint {paths, priority}`. It decodes with
  `load_image_rgb(p, max_dim=512)` and runs `fingerprint` only (no DINO, no gram), using the same
  `ThreadPoolExecutor(_VERIFY_WORKERS)` pattern as `verify`. It returns
  `{"results": [{"path", "phash": [...]} | {"path", "error": "decode-failed"}]}`.
  It does **not** require `embed_model_ready()`.
- This is additive: stored vectors are unchanged, so **do not bump `EMBED_SCHEMA_VERSION`**.

**Rust**

- `core::sidecar`:
  - Add `Descriptor.phash: Vec<u64>`. Parse the hex strings and skip bad ones. An absent field
    (older sidecar) gives an empty vec.
  - Add `VerifyResult.reason: Option<String>`. The sidecar already sends it.
  - Add `pub fn fingerprint(paths: &[String], priority: Priority) -> Result<Vec<(String, std::result::Result<Vec<u64>, String>)>>`.
- `core::database`:
  - New table in the `CREATE TABLE IF NOT EXISTS` batch:
    `file_fingerprints(file_id INTEGER PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE, phash INTEGER)`.
    `phash` is nullable: NULL means "flat image, deliberately no fingerprint", so the backfill
    does not retry it forever. Store the `u64` bit-cast `as i64` and read it back `as u64`.
  - Add `upsert_fingerprint(con, file_id, Option<u64>)`.
  - Add `fingerprints_in_folder(con, folder) -> Vec<(i64 faiss_id, u64 phash)>`: join `files`
    and use the same prefix filter as `folder_id_map`, non-null only.
  - Add `files_missing_fingerprint(con, folder) -> Vec<(i64 file_id, String path)>`.
  - Unit-test the high-bit round-trip (`0xFFFF_FFFF_FFFF_FFFF`, `1 << 63`).
- `services::sync::index_chunks` and `reembed_folder`: after a successful upsert, write
  `desc.phash.first().copied()`. Write `None` when `phash` is empty and the sidecar sent the
  field. If you can't tell "absent" from "flat", write `None`: the backfill skips NULL rows and
  the next re-index fixes them.
- Backfill: `services::sync::backfill_fingerprints(folder) -> usize`, modelled on
  `services::tags::backfill_colors`.
  - Chunks of 64 paths per `/fingerprint` call.
  - Wait on `search_gate` the same way the colour backfill does, so it never competes with a
    search.
  - Log one `[timing] fingerprint backfill done` line.
  - Spawn it next to the colour backfill in `core/watcher.rs` (after `library_sync_complete`).
  - Also run it once at startup for every watched folder, so existing 10k-image libraries are
    fingerprinted without a re-index.

**Search (`services::search::execute`)**

- Query variants come from `query_desc.phash`, which describes the same trimmed image.
- Load `fingerprints_in_folder` for each scope folder. For each file, take the minimum Hamming
  distance over the 8 query variants and remember which variant won.
- Classify with new `config.rs` constants:
  - `EXACT_HASH_MAX_DIST: u32 = 6`
  - `EXACT_COLOR_MIN_SIM: f32 = 0.90`
  - If `dist <= EXACT_HASH_MAX_DIST` and `color_sim >= EXACT_COLOR_MIN_SIM`: `"exact"`, or
    `"exactTransformed"` when the winning variant is not 0.
  - If `dist <= EXACT_HASH_MAX_DIST` and colour differs: `"colourway"` (same design, other
    colour).
  - Otherwise `"none"`.
- An exact or colourway file may be missing from stage 1 if it fell below the floor. In that case
  build its `Match` with a new `VectorStore::score_ids(&self, ids, query_embed, query_rose,
  query_gram, query_color) -> Vec<Match>`, which reuses the same per-entry scoring as
  `near_family`; factor that scoring into one private fn. Then add it to `candidates`.
- Include exact and colourway rows in the **first** partial so they paint at the top within
  milliseconds.
- If the query has no fingerprint (flat image), skip the tier.

### 3.2 Fused ranking (replaces the verified-first partition)

One `score` per row, used everywhere for ordering (backend sort and UI sort). New `config.rs`
constants (one-line comment each, plus a short block comment that these are uncalibrated
starting points to be tuned with `/test-app`):

```
RANK_EXACT_BASE: f32 = 3.0          // exact / exactTransformed rows: always first
RANK_COLOURWAY_BASE: f32 = 2.0      // same design other colour: right after exact
RANK_VERIFY_BONUS: f32 = 0.08       // max boost geometric proof can add
RANK_VERIFY_STRONG_INLIERS: u32 = 60 // inliers at which the boost saturates
RANK_REJECT_PENALTY: f32 = 0.03     // SIFT had evidence and refused
RANK_COLOR_WEIGHT: f32 = 0.06       // applied only when the request's preferColor is true
RANK_VISIBLE_WINDOW: usize = 200    // fast-tier verify only what could reach this rank
```

`fn fused_score(m: &Match, v: Option<&VerifyResult>, kind: MatchKind, prefer_color: bool) -> f32`:

- exact or exactTransformed: `RANK_EXACT_BASE + (1 - dist/64) + embed_sim * 0.01`
- colourway: `RANK_COLOURWAY_BASE + (1 - dist/64) + color_sim * 0.01`
- otherwise:
  `embed_sim + bonus + color_term - penalty`, where:
  - `bonus = RANK_VERIFY_BONUS * strength` if matched, where
    `strength = clamp((inliers - 10) / (RANK_VERIFY_STRONG_INLIERS - 10), 0, 1) * clamp(inlier_ratio / 0.6, 0.5, 1)`
  - `penalty = RANK_REJECT_PENALTY` only when not matched **and** `reason` is
    `low-confidence`, `match-outside-frame`, `implausible-warp`, `implausible-rotation` or
    `degenerate-homography`. `too-few-keypoints`, `too-few-matches`, `decode-failed`,
    `no-transform` and unchecked rows get 0. "SIFT couldn't see anything" is not evidence
    against a match.
  - `color_term = RANK_COLOR_WEIGHT * color_sim` when `prefer_color`, else 0.

Because the bonus and penalty are bounded, a 0.72-cosine false positive can never pass a
0.95-cosine true match. As verification streams in, rows move only locally instead of jumping
the whole grid.

Sort: `score` desc, then `similarity` desc, then `path` asc (deterministic). Assign `rank` after
the sort, **and on every partial too**, so rank is never 0 in a payload.

### 3.3 Faster fast tier: verify only what can reach the visible window

Replace "verify the top `min(near_family_n, VERIFY_MAX_CANDIDATES)`" in the non-Full-DB path:

1. Compute `pre = fused_score(m, None, kind, prefer_color)` for every candidate and sort by it.
2. Let `cutoff = pre[RANK_VISIBLE_WINDOW - 1]`, or −∞ if there are fewer candidates.
3. The fast-tier verify set is every candidate with `pre + RANK_VERIFY_BONUS >= cutoff`, capped
   at `VERIFY_MAX_CANDIDATES`. Skip `exact` and `colourway` rows: they are already decided. Any
   candidate outside this set cannot enter the top window even if it verifies.
4. Candidates outside the set are still returned, as `"unchecked"`. The deep tier and Full DB
   keep their current behaviour and remain the way to verify everything.
5. The mirrored retry runs over the same set.
6. Put this in a pure fn `fast_verify_count(pre_sorted: &[f32]) -> usize` and unit-test it.

### 3.4 IPC model changes (mirror both sides, camelCase)

- `models/response/response_search.rs::SearchResult` and `UI/src/app/models/response/responseSearch.ts`:
  - `score: f32`, the fused score ×100, rounded to 0.1.
  - `matchKind: String`: `"exact" | "exactTransformed" | "colourway" | "none"`.
  - `hashDistance: Option<u32>` / `hashDistance?: number | null`.
  - Fix the stale comment on `similarity`: it is the DINO embedding cosine, not rose+gram.
- `models/request/request_search.rs::RequestStartSearch` and `requestSearch.ts`:
  `prefer_color: Option<bool>` / `preferColor?: boolean`. Thread it through
  `commands/search_commands.rs` into `execute`.

### 3.5 Logging

Extend `[timing] search TOTAL` with `exact_n`, `colourway_n`, `fingerprints_n`,
`fingerprint_ms`, `fast_verify_n` and `prefer_color`. Add one `log::info!` line, `[rank] top5`,
with path basename, score, similarity, verification and matchKind for the first five rows. That
line is what lets a field report be diagnosed.

### 3.6 Result view (Angular, `UI/src/app/views/search/`)

- Delete the family/similar split (`familyResults`, `similarResults`, the two sections).
  Replace it with:
  1. **"Best matches" strip** at the top: rows with `matchKind != 'none'`, larger cards with
     badges for "Exact copy", "Rotated/mirrored copy" and "Same design · other colour".
     Hidden when empty.
  2. **One ranked grid** of the remaining rows, ordered by `score` desc (same tie-breaks as
     Rust). Each card shows a small rank number (`#12`) and its existing verification badge
     (verified N pts / found inside / mirrored / "Checking…" for unchecked / "Similar").
  3. **Filter chips** above the grid: `All` · `Verified only`. Client-side filters over the same
     array; they never re-run the search.
  4. **"Prefer same colour" toggle** next to the search button. Persist it in
     `SearchStateService`, send it as `preferColor`, default **on**.
- Grid layout: change the `i % 3` masonry columns to a row-major CSS grid
  (`grid-template-columns: repeat(auto-fill, minmax(200px, 1fr))`, fixed-height thumb box with
  `object-fit: contain`). Reading order then equals rank order. Keep "Show more" paging
  (`SIMILAR_PAGE`) on the grid.
- `track item.path` (not `item.rank`) everywhere.
- `resortResults()` sorts by `score` desc, then `similarity` desc, then `path`.
- i18n: add keys to `UI/src/assets/i18n/en.json` (and every other locale file in that folder,
  using the English text if there is no translation). Remove keys that are no longer used.
- Go through `ZoneWrapperService` only; follow `ui-framework.md` and the existing PrimeNG
  usage.

### 3.7 Docs to update in the same change

- `.claude/rules/search-pipeline.md`: add stage 0 (fingerprint tier), fused ranking and the
  visible-window verify bound.
- `.claude/rules/sidecar.md`: add `/fingerprint` to the endpoints table.
- `CLAUDE.md` cross-file invariants: the `RANK_*` constants are uncalibrated, like
  `NEAR_FAMILY_MIN_SIM`.

## 4. Phase 3 — proposed, NOT implemented here

- **Calibrate** `NEAR_FAMILY_MIN_SIM`, `EXACT_HASH_MAX_DIST` and `RANK_*` from real 10k-library
  logs with `/test-app` (`[rank] top5` plus `[calibration]` lines), then set
  `CALIBRATION_LOGGING_ENABLED = false`.
- **Cached local features**: store compact local descriptors per file at index time, so verify
  no longer decodes the candidate and re-runs SIFT. That is the dominant per-candidate cost.
  Alternatively, replace BF-SIFT with a learned matcher (ALIKED/SuperPoint + LightGlue, ONNX).
- **Copy-detection embedding**: a model trained for near-duplicates, such as SSCD (check the
  licence: SSCD weights are CC-BY-NC) or DINOv2/v3 patch-token re-ranking. Requires an
  `EMBED_SCHEMA_VERSION` bump and a re-embed.
- **ANN index** (HNSW or IVF-PQ) once libraries reach about 200k files. At 10k, a brute-force dot
  product over 10k × 9 × 1536 is milliseconds-scale and not the bottleneck.
- **Learning from clicks**: a "this is the one" action on a card logs (query, chosen) pairs, and
  the `RANK_*` weights are fitted offline from them.

## 5. Acceptance

- `cargo test` passes, including new tests for: hex phash parse (values ≥ 2^53), Hamming min over
  variants, `fused_score` ordering (exact > colourway > verified 0.99 > rejected-with-no-keypoints
  0.98 > verified 0.72; a bounded bonus can't flip 0.72 over 0.95), the reject penalty applying
  only to evidence reasons, `fast_verify_count`, and the DB fingerprint round-trip.
- `npm test -- --watch=false` passes. Add a spec for the client sort and filter if the view
  logic is factored into a pure function (preferred).
- `npm run build` succeeds.
- Sidecar: `fingerprint` of an image and of its `np.rot90` copy are within Hamming 4 at the
  matching variant index. A flat image returns `[]`.
- Comment style follows `.claude/rules/code-format.md`: one line, no `///` or JSDoc.
