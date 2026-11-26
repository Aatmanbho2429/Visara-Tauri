// Search execution — describes the query via the sidecar, selects the near
// family by DINO embedding cosine over every file in scope (in-process), then
// geometrically verifies a bounded, cosine-ordered prefix of it through the
// sidecar before returning results.
//
// Membership is a similarity floor, not a rank cut: everything at or above
// `config::NEAR_FAMILY_MIN_SIM` is returned, however many that is. SIFT/RANSAC
// then splits the *verified prefix* of that family into a verified tier (a
// real geometric proof, which is what "found inside" and a mirrored match
// mean) and a rejected tail; whatever sits beyond the verify budget
// (`config::VERIFY_MAX_CANDIDATES` / `VERIFY_TIME_BUDGET_MS`) comes back
// `verification: "unchecked"` rather than being silently dropped or read as
// rejected.
//
// Results stream to the caller as they're known: an all-`"unchecked"` set the
// moment stage 1 finishes, then per-chunk updates as verification proves or
// rejects each candidate — see `VerifyProgress`.
//
// A deep tier then keeps verifying the band below that floor in the background, returning only rows
// that verify.
//
// Multi-folder model unchanged: empty `scope` means every watched folder.

use super::match_tier::{self, MatchKind};
use crate::{
    config::{
        match_engine_mode, CALIBRATION_LOGGING_ENABLED, DEEP_VERIFY_DEFAULT_MIN_SIM, DEEP_VERIFY_MAX_CANDIDATES,
        DEEP_VERIFY_MIN_SIM_FLOOR, DEEP_VERIFY_TIME_BUDGET_MS, EXACT_HASH_MAX_DIST, MatchEngineMode, NEAR_FAMILY_MIN_SIM,
        VECTOR_STORE_PATH, VERIFY_MAX_CANDIDATES, VERIFY_TIME_BUDGET_MS,
    },
    core::{database, feature_store, search_gate, sidecar, vector_store},
    error::{Result, PictoriaError},
    services::match_engine,
    models::response::{FailedFile, MatchRegion, SearchResult},
    utils::image_loader,
};
use image::DynamicImage;
use std::{
    cmp::Ordering as CmpOrdering,
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    sync::Arc,
    time::{Duration, Instant},
};

// Set by `search_cancel` (or a replacing `search_start`); `verify_pass` stops at the next chunk boundary.
pub static SEARCH_CANCEL: AtomicBool = AtomicBool::new(false);

// ── Public API ────────────────────────────────────────────────────────────

// The verify pool is no longer a fixed shortlist. `VectorStore::near_family`
// returns every file whose DINO embedding cosine clears
// `config::NEAR_FAMILY_MIN_SIM` — membership is decided by similarity rather
// than a rank cut. What IS bounded is how much of that family gets
// SIFT/RANSAC-verified in one search: `VERIFY_MAX_CANDIDATES` caps the count
// and `VERIFY_TIME_BUDGET_MS` caps the wall clock, taken from the
// highest-cosine prefix since `candidates` is already sorted that way. An
// unbounded verify pass is the entire reason a search could take 79s.
//
// The old fixed top-1500 rank cut this replaced had the property that a real
// match could be excluded purely because a library held more than 1500 tiles
// closer to the query — invisible from the outside, and unfixable by the
// user. `near_family_n` in the timing log below is the number to watch if a
// search ever feels slow; a large fraction of the library clearing the floor
// means the floor, not the verify budget, is what needs recalibrating (see
// `VectorStore::cosine_histogram`).

// Verify runs in chunks of this size rather than one request for the whole
// shortlist, purely so `on_verify_progress` below has something to report
// between chunks — SIFT/RANSAC on a 200-candidate shortlist takes tens of
// seconds even parallelized server-side (see `sidecar/pipeline.py`), and a
// single request that only resolves at the very end left the UI's progress
// bar sitting frozen at 0% for the whole wait, indistinguishable from hung.
//
// Raised from 20 to 100: the sidecar
// re-decodes and re-SIFTs the *query* once per HTTP request
// (`pipeline.py::prepare_query`), so a 213-candidate family at chunk size 20
// paid for that 11 times — 22 with a mirrored pass. `sidecar::verify_timeout`
// scales as `30 + n_candidates` seconds, so this raises the per-chunk ceiling
// to 130s, which is fine: progress granularity is preserved because results
// stream per chunk regardless of chunk size (see `VerifyProgress`).
const VERIFY_CHUNK: usize = 100;

// A verified match is "found inside" rather than "same image" when its
// matched region covers less than this fraction of the candidate's area.
const PARTIAL_MAX_AREA_FRACTION: f32 = 0.85;

// Cosine thresholds the calibration log reports population counts
// at, in addition to `NEAR_FAMILY_MIN_SIM` itself. Diagnostic only — nothing
// here changes what a search returns.
const CALIBRATION_BUCKETS: [f32; 8] = [0.60, 0.65, 0.70, 0.75, 0.80, 0.85, 0.90, 0.95];

// One member of the near family, resolved against the DB and de-duplicated —
// built once, before verification, and updated in place as verify chunks
// return so both the initial "unchecked" set and every later update come
// from the same construction path.
struct Candidate {
    m: crate::core::vector_store::Match,
    path: String,
    folder: String,
    // pHash verdict (`MatchKind::None` when the fingerprint tier did not claim this file).
    kind: MatchKind,
    // Hamming distance behind `kind`; only set for the three hash tiers.
    hash_distance: Option<u32>,
}

// Reported after every verify chunk (including a zero-progress call before
// the first one starts). `updated` carries the `SearchResult`s that changed
// in this tick — freshly verified or rejected — so the caller can stream
// them on rather than wait for the whole search to finish; it's empty for
// the very first, all-`"unchecked"` call. `done`/`total` span every pass, and
// `tier` (`"near"`, `"mirrored"`, `"deep"`) tells the UI which one is running.
pub struct VerifyProgress<'a> {
    pub done: usize,
    pub total: usize,
    pub tier: &'static str,
    pub updated: &'a [SearchResult],
}

// Everything `execute` produces: the result set plus, in shadow mode, the inputs for the post-complete engine run.
pub struct SearchOutput {
    pub results: Vec<SearchResult>,
    pub failed_files: Vec<FailedFile>,
    pub shadow: Option<ShadowInput>,
}

// What the shadow run needs after the search has shipped; owns the temp query file so it outlives `execute`.
pub struct ShadowInput {
    _temp: TempQueryFile,
    store: Arc<vector_store::VectorStore>,
    scope_paths: Vec<String>,
    query_path: String,
    variants: Vec<u64>,
    descs: QueryDescriptors,
    id_map: HashMap<i64, (String, String)>,
    query_abs: String,
    legacy_results: Vec<SearchResult>,
    // The first `SHADOW_LEGACY_VERIFY` paths the legacy pass verified, re-checked here from stored features.
    legacy_verify_paths: Vec<String>,
    legacy_verify_ms: f64,
}

// How many of the legacy verify set the shadow run re-verifies from stored features.
const SHADOW_LEGACY_VERIFY: usize = 100;

// What a shadow run measured, logged as one `[match_engine] shadow` line.
#[derive(Debug, Default)]
struct ShadowReport {
    exact_n: usize,
    colourway_n: usize,
    copy_n: usize,
    engine_only: Vec<String>,
    legacy_only: Vec<String>,
    agree_n: usize,
    feature_verify_ms: f64,
    legacy_verify_ms: f64,
    shortlist_n: usize,
    fallback_n: usize,
    vocab_ready: bool,
    query_features_ms: f64,
    retrieve_ms: f64,
    stage0_ms: f64,
    fingerprints_n: usize,
    total_ms: f64,
}

// Shadow mode: runs the engine's stages against the same query after `search_complete` and only logs what it found.
pub fn run_shadow(input: ShadowInput) {
    if let Some(r) = shadow_report(&input) {
        log::info!(
            "[match_engine] shadow exact_n={} colourway_n={} copy_n={} engine_only={:?} legacy_only={:?} agree_n={} \
             feature_verify_ms={:.2} legacy_verify_ms={:.2} shortlist_n={} fallback_n={} vocab_ready={} \
             query_features_ms={:.2} retrieve_ms={:.2} stage0_ms={:.2} fingerprints_n={} total_ms={:.2}",
            r.exact_n, r.colourway_n, r.copy_n, r.engine_only, r.legacy_only, r.agree_n, r.feature_verify_ms,
            r.legacy_verify_ms, r.shortlist_n, r.fallback_n, r.vocab_ready, r.query_features_ms, r.retrieve_ms,
            r.stage0_ms, r.fingerprints_n, r.total_ms,
        );
    }
}

// Runs the shadow stages and returns what they measured; `None` when a newer search cancelled it or stage 0 failed.
fn shadow_report(input: &ShadowInput) -> Option<ShadowReport> {
    if SEARCH_CANCEL.load(Ordering::Relaxed) {
        return None;
    }
    // Holding the gate keeps background backfills out of the way while this runs.
    let _gate = search_gate::begin();
    let t0 = Instant::now();
    let basename = |p: &str| Path::new(p).file_name().and_then(|n| n.to_str()).unwrap_or(p).to_string();

    // Stage 0 and stage 0b, exactly as primary would run them.
    let t_stage0 = Instant::now();
    let shortlist = match_engine::retrieve_shortlist(&input.query_path, sidecar::Priority::Index);
    let shortlist_ids: HashSet<i64> = shortlist.hits.iter().map(|(id, _)| *id).collect();
    let s0 = match collect_hits(
        &input.store,
        &input.scope_paths,
        &input.variants,
        &shortlist_ids,
        &input.descs,
        &HashMap::new(),
        &input.id_map,
        &input.query_abs,
    ) {
        Ok(s) => s,
        Err(e) => {
            log::warn!("[match_engine] shadow stage 0 failed: {e}");
            return None;
        }
    };
    let stage0_ms = t_stage0.elapsed().as_secs_f64() * 1000.0;
    let exact_n = s0.hits.iter().filter(|c| matches!(c.kind, MatchKind::Exact | MatchKind::ExactTransformed)).count();
    let colourway_n = s0.hits.iter().filter(|c| c.kind == MatchKind::Colourway).count();

    // The set both sides are compared on: the engine shortlist plus the head of the legacy verify set.
    let mut ids: HashMap<String, i64> = HashMap::new();
    let mut paths: Vec<String> = Vec::new();
    for c in &s0.hits {
        if shortlist_ids.contains(&c.m.id) && ids.insert(c.path.clone(), c.m.id).is_none() {
            paths.push(c.path.clone());
        }
    }
    let legacy_by_path: HashMap<&str, &SearchResult> = input.legacy_results.iter().map(|r| (r.path.as_str(), r)).collect();
    let path_to_id: HashMap<&str, i64> = input.id_map.iter().map(|(id, (p, _))| (p.as_str(), *id)).collect();
    for p in input.legacy_verify_paths.iter().take(SHADOW_LEGACY_VERIFY) {
        if let Some(&id) = path_to_id.get(p.as_str()) {
            if ids.insert(p.clone(), id).is_none() {
                paths.push(p.clone());
            }
        }
    }
    let has: Vec<bool> = feature_store::with_store(|s| paths.iter().map(|p| s.contains(ids[p])).collect()).unwrap_or_default();
    let (feat_paths, fallback_paths) = split_by_flags(&paths, &has);

    // Straight, then mirrored for whatever did not match; stored features make both cheap.
    let t_verify = Instant::now();
    let mut matched: HashMap<String, sidecar::VerifyResult> = HashMap::new();
    for mirror in [false, true] {
        let todo: Vec<String> = feat_paths.iter().filter(|p| !matched.contains_key(*p)).cloned().collect();
        for chunk in todo.chunks(VERIFY_CHUNK) {
            if SEARCH_CANCEL.load(Ordering::Relaxed) {
                return None;
            }
            let stored = read_stored_features(chunk, &ids);
            match sidecar::verify_features(&input.query_path, &stored, mirror, sidecar::Priority::Index) {
                Ok(rows) => matched.extend(rows.into_iter().filter(|(_, r)| r.matched)),
                Err(e) => log::warn!("[match_engine] shadow verify chunk failed: {e}"),
            }
        }
    }
    let feature_verify_ms = t_verify.elapsed().as_secs_f64() * 1000.0;

    let copy: HashSet<&str> =
        matched.iter().filter(|(_, r)| match_tier::is_copy(r.matched, r.inliers, r.inlier_ratio)).map(|(p, _)| p.as_str()).collect();
    let in_set: HashSet<&str> = feat_paths.iter().map(|p| p.as_str()).collect();
    let legacy_verified: HashSet<&str> =
        in_set.iter().copied().filter(|p| legacy_by_path.get(p).is_some_and(|r| r.verified)).collect();
    let engine_matched: HashSet<&str> = matched.keys().map(|p| p.as_str()).collect();
    let mut engine_only: Vec<String> = engine_matched.difference(&legacy_verified).map(|p| basename(p)).collect();
    let mut legacy_only: Vec<String> = legacy_verified.difference(&engine_matched).map(|p| basename(p)).collect();
    engine_only.sort();
    legacy_only.sort();
    Some(ShadowReport {
        exact_n,
        colourway_n,
        copy_n: copy.len(),
        engine_only,
        legacy_only,
        agree_n: engine_matched.intersection(&legacy_verified).count(),
        feature_verify_ms,
        legacy_verify_ms: input.legacy_verify_ms,
        shortlist_n: shortlist.hits.len(),
        fallback_n: fallback_paths.len(),
        vocab_ready: shortlist.vocab_ready,
        query_features_ms: shortlist.query_features_ms,
        retrieve_ms: shortlist.retrieve_ms,
        stage0_ms,
        fingerprints_n: s0.fingerprints_n,
        total_ms: t0.elapsed().as_secs_f64() * 1000.0,
    })
}

// What one `verify_pass` run learned, beyond the raw sidecar responses:
// `attempted` is every path a chunk actually completed for (whether it
// matched or not) — distinct from a path that was never reached, which must
// read as "unchecked", not "rejected". `any_chunk_failed` and
// `budget_exhausted` both gate the mirrored retry below (see 1c).
struct VerifyPassResult {
    by_path: HashMap<String, sidecar::VerifyResult>,
    attempted: HashSet<String>,
    any_chunk_failed: bool,
    budget_exhausted: bool,
    cancelled: bool,
    // Candidates this pass got through, so the caller can keep one cumulative counter.
    done: usize,
    // Candidates sent in chunks that failed — never verified, and silently dropped in the deep tier.
    failed_n: usize,
}

// Every bound one search runs under; `None` / `usize::MAX` mean unbounded (Full DB).
#[derive(Debug, PartialEq)]
struct VerifyLimits {
    stage1_floor: f32,
    tier_a_max: usize,
    tier_a_budget: Option<Duration>,
    deep_max: usize,
    deep_budget: Option<Duration>,
    deep_on: bool,
}

// Full DB drops the floor, both caps and both budgets; otherwise these are the configured bounds.
fn verify_limits(deep_min_sim: f32, full_db: bool) -> VerifyLimits {
    if full_db {
        return VerifyLimits {
            stage1_floor: f32::NEG_INFINITY,
            tier_a_max: usize::MAX,
            tier_a_budget: None,
            deep_max: usize::MAX,
            deep_budget: None,
            deep_on: true,
        };
    }
    VerifyLimits {
        stage1_floor: stage1_floor(deep_min_sim),
        tier_a_max: VERIFY_MAX_CANDIDATES,
        tier_a_budget: Some(Duration::from_millis(VERIFY_TIME_BUDGET_MS)),
        deep_max: DEEP_VERIFY_MAX_CANDIDATES,
        deep_budget: Some(Duration::from_millis(DEEP_VERIFY_TIME_BUDGET_MS)),
        deep_on: deep_enabled(deep_min_sim),
    }
}

// How a verify pass checks each chunk: the sidecar decodes every candidate (`/verify`), or matches features stored at index time (`/verify_features`).
#[derive(Clone, Copy)]
enum VerifyMode<'a> {
    Decode,
    // `ids` maps a candidate path to the vector id its features are stored under.
    Features { ids: &'a HashMap<String, i64> },
}

// Reads the stored features for a chunk of paths; a path whose record has gone missing is left out and reads as unverified.
fn read_stored_features(chunk: &[String], ids: &HashMap<String, i64>) -> Vec<(String, crate::core::feature_store::Features)> {
    crate::core::feature_store::with_store(|s| {
        chunk.iter().filter_map(|p| ids.get(p).and_then(|id| s.read(*id)).map(|f| (p.clone(), f))).collect()
    })
    .unwrap_or_default()
}

// Folds a second pass's outcome into the first, as if it were one pass over both sets.
fn merge_passes(mut a: VerifyPassResult, b: VerifyPassResult) -> VerifyPassResult {
    a.by_path.extend(b.by_path);
    a.attempted.extend(b.attempted);
    a.any_chunk_failed |= b.any_chunk_failed;
    a.budget_exhausted |= b.budget_exhausted;
    a.cancelled |= b.cancelled;
    a.done += b.done;
    a.failed_n += b.failed_n;
    a
}

// One orientation over both candidate groups: stored-feature candidates first, then the decode fallback with whatever budget is left.
#[allow(clippy::too_many_arguments)]
fn verify_sets(
    query_path: &str,
    feat_paths: &[String],
    fallback_paths: &[String],
    ids: &HashMap<String, i64>,
    mirror: bool,
    budget: Option<Duration>,
    cancel: &AtomicBool,
    mut on_chunk: impl FnMut(usize, usize, bool, &HashMap<String, sidecar::VerifyResult>),
) -> VerifyPassResult {
    let total = feat_paths.len() + fallback_paths.len();
    let started = Instant::now();
    let first = verify_pass(query_path, feat_paths, mirror, VerifyMode::Features { ids }, budget, cancel, |d, _, m, c| {
        on_chunk(d, total, m, c)
    });
    if first.cancelled || first.budget_exhausted || fallback_paths.is_empty() {
        return first;
    }
    let base = first.done;
    let left = budget.map(|b| b.saturating_sub(started.elapsed()));
    let second = verify_pass(query_path, fallback_paths, mirror, VerifyMode::Decode, left, cancel, |d, _, m, c| {
        on_chunk(base + d, total, m, c)
    });
    merge_passes(first, second)
}

// Run `paths` through the sidecar in chunks, stopping once `budget` elapses
// — whatever has been proven by then is what's returned, and the remainder
// is left for the caller to mark `"unchecked"`. `on_chunk` fires once before
// any work (a pure progress tick, empty chunk map) and once per completed
// chunk after.
//
// A chunk that fails degrades those candidates to "unchecked" (not
// "rejected" — SIFT never actually ran) rather than failing the whole
// search, but it's *logged* and tracked in `any_chunk_failed` rather than
// silently swallowed: the mirrored retry below triggers on "nothing
// verified", and a transport failure produces exactly that state while
// meaning something completely different.
fn verify_pass(
    query_path: &str,
    paths: &[String],
    mirror: bool,
    mode: VerifyMode,
    // `None` means no deadline (Full DB); `Instant + Duration::MAX` would panic, so it can't be a huge value.
    budget: Option<Duration>,
    cancel: &AtomicBool,
    mut on_chunk: impl FnMut(usize, usize, bool, &HashMap<String, sidecar::VerifyResult>),
) -> VerifyPassResult {
    let total = paths.len();
    let mut out = HashMap::with_capacity(total);
    let mut attempted = HashSet::with_capacity(total);
    let mut any_chunk_failed = false;
    let mut budget_exhausted = false;
    let mut cancelled = false;
    let mut done = 0usize;
    let mut failed_n = 0usize;
    let deadline = budget.map(|b| Instant::now() + b);

    on_chunk(0, total, mirror, &HashMap::new());
    for chunk in paths.chunks(VERIFY_CHUNK) {
        if deadline.is_some_and(|d| Instant::now() >= d) {
            budget_exhausted = true;
            log::warn!(
                "[search] verify budget ({budget:?}) exhausted after {done}/{total} \
                 candidates (mirror={mirror}) — remainder left unchecked, not rejected"
            );
            break;
        }
        if cancel.load(Ordering::Relaxed) {
            cancelled = true;
            log::info!("[search] verify cancelled after {done}/{total} candidates (mirror={mirror})");
            break;
        }
        let mut chunk_out: HashMap<String, sidecar::VerifyResult> = HashMap::with_capacity(chunk.len());
        let called = match mode {
            VerifyMode::Decode => sidecar::verify(query_path, chunk, mirror, sidecar::Priority::Search),
            VerifyMode::Features { ids } => {
                let stored = read_stored_features(chunk, ids);
                sidecar::verify_features(query_path, &stored, mirror, sidecar::Priority::Search)
            }
        };
        match called {
            Ok(results) => {
                for (path, result) in results {
                    attempted.insert(path.clone());
                    chunk_out.insert(path, result);
                }
            }
            Err(e) => {
                any_chunk_failed = true;
                failed_n += chunk.len();
                log::warn!(
                    "[search] verify chunk failed (n={}, mirror={mirror}): {e} — \
                     those candidates stay unchecked, not rejected",
                    chunk.len(),
                );
            }
        }
        done += chunk.len();
        on_chunk(done, total, mirror, &chunk_out);
        out.extend(chunk_out);
    }

    VerifyPassResult { by_path: out, attempted, any_chunk_failed, budget_exhausted, cancelled, done, failed_n }
}

// Build (or rebuild) one result row from a candidate and, if verification has
// reached it yet, the sidecar's verdict. `verification` is the caller's
// classification for this call — `"unchecked"`, `"verified"`, or
// `"rejected"` — kept as a separate argument rather than derived from `v`
// alone so a chunk failure (verified stays `None` but was still "attempted"
// in the sense of being given up on) can still be expressed correctly.
fn build_result(c: &Candidate, v: Option<&sidecar::VerifyResult>, verification: &str) -> SearchResult {
    let name = Path::new(&c.path).file_name().and_then(|n| n.to_str()).unwrap_or(&c.path).to_string();
    let verified = verification == "verified";
    let scale = |x: f32| (x.clamp(0.0, 1.0) * 100.0 * 10.0).round() / 10.0;

    let (partial, match_region) = match v.filter(|_| verified) {
        Some(r) => match (r.quad, r.candidate_size) {
            (Some(quad), Some(size)) => {
                let region = quad_to_fraction_box(&quad, size);
                let area_fraction = region.w * region.h;
                (area_fraction < PARTIAL_MAX_AREA_FRACTION, region)
            }
            _ => (false, MatchRegion { x: 0.0, y: 0.0, w: 1.0, h: 1.0 }),
        },
        None => (false, MatchRegion { x: 0.0, y: 0.0, w: 1.0, h: 1.0 }),
    };

    // The pHash verdict stands; otherwise a strong verified match is a copy, but only when the engine is primary.
    let kind = match (c.kind, v) {
        (MatchKind::None, Some(r))
            if verified && match_engine_mode() == MatchEngineMode::Primary && match_tier::is_copy(r.matched, r.inliers, r.inlier_ratio) =>
        {
            MatchKind::Copy
        }
        (k, _) => k,
    };

    SearchResult {
        rank: 0,
        path: c.path.clone(),
        name,
        // Both are the embedding cosine — the same number
        // `NEAR_FAMILY_MIN_SIM` filters on, so "70% pattern match" in the UI
        // means exactly what the threshold meant.
        similarity: scale(c.m.embed_sim),
        pattern_match: scale(c.m.embed_sim),
        color_match: scale(c.m.color_sim),
        folder: c.folder.clone(),
        verified,
        verification: verification.to_string(),
        partial,
        match_region,
        match_points: if verified { v.map(|r| r.inliers).unwrap_or(0) } else { 0 },
        mirrored: verified && v.map(|r| r.mirrored).unwrap_or(false),
        match_kind: kind.as_str().to_string(),
        hash_distance: c.hash_distance,
    }
}

// Apply one verify chunk's results onto `results` in place, returning the
// indices of the rows that changed so the caller can stream just those on
// without re-sending everything. `mirror_hits_only`, set for the mirrored pass,
// keeps a straight-pass miss's data intact — a mirrored miss carries no more
// information than the straight miss already recorded, and overwriting
// would discard the first pass's inlier counts for no reason.
fn apply_chunk(
    results: &mut [SearchResult],
    path_index: &HashMap<String, usize>,
    candidates: &[Candidate],
    chunk_out: &HashMap<String, sidecar::VerifyResult>,
    mirror_hits_only: bool,
) -> Vec<usize> {
    let mut updated = Vec::with_capacity(chunk_out.len());
    for (path, v) in chunk_out {
        if mirror_hits_only && !v.matched {
            continue;
        }
        if let Some(&i) = path_index.get(path) {
            let verification = if v.matched { "verified" } else { "rejected" };
            results[i] = build_result(&candidates[i], Some(v), verification);
            updated.push(i);
        }
    }
    updated
}

// Resolves the request's `deepMinSim` into the floor the search uses: absent or non-finite means the default, anything else is clamped.
pub fn resolve_deep_min_sim(requested: Option<f32>) -> f32 {
    match requested {
        Some(v) if v.is_finite() => v.clamp(DEEP_VERIFY_MIN_SIM_FLOOR, NEAR_FAMILY_MIN_SIM),
        _ => DEEP_VERIFY_DEFAULT_MIN_SIM,
    }
}

// The deep tier only exists when the user asked to verify below the fast floor.
fn deep_enabled(deep_min_sim: f32) -> bool {
    deep_min_sim < NEAR_FAMILY_MIN_SIM
}

// Stage 1 runs once, at the lower of the two floors, and is split afterwards.
fn stage1_floor(deep_min_sim: f32) -> f32 {
    deep_min_sim.min(NEAR_FAMILY_MIN_SIM)
}

// How many leading candidates are Tier A (cosine at or above the fast floor); `sims` is sorted descending.
fn tier_a_count(sims: &[f32]) -> usize {
    sims.partition_point(|&s| s >= NEAR_FAMILY_MIN_SIM)
}

// Applies a chunk for "late" candidates (deep band and shortlist-only extras): only rows that verified are built, and each
// joins `results` once (`pos` remembers where) so a later pass updates it in place; returns the indices that changed.
fn apply_late_chunk(
    results: &mut Vec<SearchResult>,
    late_index: &HashMap<String, usize>,
    pos: &mut HashMap<String, usize>,
    candidates: &[Candidate],
    chunk_out: &HashMap<String, sidecar::VerifyResult>,
) -> Vec<usize> {
    let mut updated = Vec::new();
    for (path, v) in chunk_out {
        if !v.matched {
            continue;
        }
        let Some(&i) = late_index.get(path) else { continue };
        let row = build_result(&candidates[i], Some(v), "verified");
        match pos.get(path) {
            Some(&r) => {
                results[r] = row;
                updated.push(r);
            }
            None => {
                results.push(row);
                pos.insert(path.clone(), results.len() - 1);
                updated.push(results.len() - 1);
            }
        }
    }
    updated
}

// Ranks `results` in place, then clones the rows at `idx` so a partial carries real (never 0) ranks.
fn ranked_updates(results: &mut [SearchResult], idx: &[usize], cmp: fn(&SearchResult, &SearchResult) -> CmpOrdering) -> Vec<SearchResult> {
    match_tier::assign_ranks(results, cmp);
    idx.iter().map(|&i| results[i].clone()).collect()
}

// Folds fingerprint/engine hits into the stage-1 candidates: a hit already in tier A only gains its verdict,
// one in the deep band moves out of it, and the rest are returned as extras (never touching tier A's cosine order).
fn merge_extras(tier_a: &mut [Candidate], deep_band: &mut Vec<Candidate>, hits: Vec<Candidate>) -> Vec<Candidate> {
    let a_index: HashMap<i64, usize> = tier_a.iter().enumerate().map(|(i, c)| (c.m.id, i)).collect();
    let hit_ids: HashSet<i64> = hits.iter().map(|h| h.m.id).collect();
    deep_band.retain(|c| !hit_ids.contains(&c.m.id));
    let mut extras = Vec::new();
    for h in hits {
        match a_index.get(&h.m.id) {
            Some(&i) => {
                if h.kind != MatchKind::None {
                    tier_a[i].kind = h.kind;
                    tier_a[i].hash_distance = h.hash_distance;
                }
            }
            None => extras.push(h),
        }
    }
    extras
}

// The query's descriptors as the engine stages need them.
struct QueryDescriptors {
    embed: Vec<f32>,
    rose: Vec<f32>,
    gram: Vec<f32>,
    color: Vec<f32>,
}

// What stages 0 and 0b found for one query.
struct Stage0 {
    hits: Vec<Candidate>,
    fingerprints_n: usize,
}

// Splits `paths` by a parallel flag list into (flagged, unflagged), keeping order.
fn split_by_flags(paths: &[String], flags: &[bool]) -> (Vec<String>, Vec<String>) {
    let mut yes = Vec::new();
    let mut no = Vec::new();
    for (i, p) in paths.iter().enumerate() {
        if flags.get(i).copied().unwrap_or(false) { yes.push(p.clone()) } else { no.push(p.clone()) }
    }
    (yes, no)
}

// Stage 0 + 0b candidates: files within `EXACT_HASH_MAX_DIST` of any query variant (classified by colour) plus the
// engine shortlist (verdict `None`), each scored against the query. `known` holds stage-1 scores to reuse.
#[allow(clippy::too_many_arguments)]
fn collect_hits(
    store: &vector_store::VectorStore,
    scope_paths: &[String],
    variants: &[u64],
    shortlist: &HashSet<i64>,
    q: &QueryDescriptors,
    known: &HashMap<i64, vector_store::Match>,
    id_map: &HashMap<i64, (String, String)>,
    query_abs: &str,
) -> Result<Stage0> {
    let mut best: HashMap<i64, (u32, usize)> = HashMap::new();
    let mut fingerprints_n = 0usize;
    if !variants.is_empty() {
        let con = database::open()?;
        for folder in scope_paths {
            for (id, hash) in database::fingerprints_in_folder(&con, folder)? {
                fingerprints_n += 1;
                if let Some((d, v)) = match_tier::min_hamming(variants, hash) {
                    if d <= EXACT_HASH_MAX_DIST {
                        best.insert(id, (d, v));
                    }
                }
            }
        }
    }
    let wanted: HashSet<i64> = best.keys().copied().chain(shortlist.iter().copied()).collect();
    let missing: Vec<i64> = wanted.iter().copied().filter(|id| !known.contains_key(id)).collect();
    let mut scored: HashMap<i64, vector_store::Match> = store
        .score_ids(&missing, &q.embed, &q.rose, &q.gram, &q.color)
        .into_iter()
        .map(|m| (m.id, m))
        .collect();
    scored.extend(known.iter().filter(|(id, _)| wanted.contains(id)).map(|(id, m)| (*id, *m)));

    let mut hits = Vec::new();
    for id in wanted {
        let (Some(m), Some((path, folder))) = (scored.get(&id), id_map.get(&id)) else { continue };
        if path == query_abs || !Path::new(path).exists() {
            continue;
        }
        let (kind, hash_distance) = match best.get(&id) {
            Some(&(dist, variant)) => (match_tier::classify_hash(dist, variant, m.color_sim), Some(dist)),
            None => (MatchKind::None, None),
        };
        hits.push(Candidate { m: *m, path: path.clone(), folder: folder.clone(), kind, hash_distance });
    }
    Ok(Stage0 { hits, fingerprints_n })
}

// `on_verify_progress` fires once before verification starts (the initial
// all-`"unchecked"` set) and once per verify chunk after — see
// `VerifyProgress`. `deep_min_sim` below `NEAR_FAMILY_MIN_SIM` adds the deep tier;
// `full_db` verifies every file in scope with no floor, cap or budget.
pub fn execute(
    image_path: &Path,
    scope: &[PathBuf],
    // What the client asked to display. No longer trims the result set — every
    // near-family member is returned and the client paginates — but it is still
    // logged, because a large gap between it and `results` is the first thing
    // to look at if the UI feels heavy after a broad search.
    top_k: usize,
    deep_min_sim: f32,
    full_db: bool,
    mut on_verify_progress: impl FnMut(VerifyProgress),
) -> Result<SearchOutput> {
    let t_total = Instant::now();
    let limits = verify_limits(deep_min_sim, full_db);
    let mode = match_engine_mode();
    let primary = mode == MatchEngineMode::Primary;
    // Ordering for ranks, partials and the final sort: tiers in primary mode, the pre-engine order otherwise.
    let rank_cmp: fn(&SearchResult, &SearchResult) -> CmpOrdering =
        if primary { match_tier::compare_results } else { match_tier::legacy_compare };

    // Pause background maintenance (the colour-tag backfill) for the duration —
    // it saturates the same cores the sidecar needs and can stall a search by
    // orders of magnitude. Released on every exit path, including the `?`s below.
    let _search_guard = search_gate::begin();

    if !sidecar::is_ready() {
        return Err(PictoriaError::ModelNotReady);
    }
    if !image_path.exists() {
        return Err(PictoriaError::Fatal("The selected reference image no longer exists.".into()));
    }

    // ── Resolve scope ─────────────────────────────────────────────────
    let t_scope = Instant::now();
    let con = database::open()?;
    let scope_paths: Vec<String> = if scope.is_empty() {
        database::watched_folder_paths(&con)?
    } else {
        scope.iter().map(|p| p.to_string_lossy().to_string()).collect()
    };
    drop(con);
    if scope_paths.is_empty() {
        return Err(PictoriaError::Fatal("No folders to search.  Add a folder to your Library first.".into()));
    }
    let scope_ms = t_scope.elapsed().as_secs_f64() * 1000.0;

    // ── Prepare the query: trim a scan/product-shot margin if there is one,
    //    so the query vector describes the design, not the background ─────
    let t_prep = Instant::now();
    let query_img = image_loader::load_image_detailed(image_path)?;
    let (orig_w, orig_h) = (query_img.width(), query_img.height());
    let query_img = trim_uniform_border(query_img);

    // The sidecar takes a path, not raw pixels — write the trimmed image to
    // a temp file only when trimming actually changed anything; otherwise
    // just point it at the original.
    let trimmed = query_img.width() != orig_w || query_img.height() != orig_h;
    let (query_sidecar_path, temp_guard) = query_path_for_sidecar(image_path, &query_img, trimmed)?;
    let prep_ms = t_prep.elapsed().as_secs_f64() * 1000.0;

    let t_describe = Instant::now();
    let described = sidecar::describe(&[query_sidecar_path.clone()], sidecar::Priority::Search)?;
    let describe_ms = t_describe.elapsed().as_secs_f64() * 1000.0;
    let query_desc = described
        .into_iter()
        .next()
        .and_then(|(_, d)| d.ok())
        .ok_or_else(|| PictoriaError::Fatal("Could not describe the reference image.".into()))?;
    let query_phash = query_desc.phash.clone();
    let query_embed: Vec<f32> = query_desc.embed.into_iter().flatten().collect();
    let query_gram: Vec<f32> = query_desc.gram.into_iter().flatten().collect();
    // Colour histogram is computed sidecar-side now (same decode as rose/gram —
    // see pipeline.color_histogram), not a separate Rust-side pass over
    // `query_img`. `query_sidecar_path` points at the same (possibly trimmed)
    // image `query_img` represents, so this is the same content either way.
    let query_color = query_desc.color;

    // ── Stage 1: rank every file in scope ──────────────────────────────
    // `store` and `stage1` both need to outlive this block (used again below
    // for `store.live_count()`/`cosine_histogram` and `stage1.first()`), but
    // the read guard must NOT — holding it
    // across the whole, multi-second verify phase below blocks the indexer's
    // `store.save()` in `core::watcher` for that entire window. Scoping the
    // guard to just this block, rather than the rest of the function, is
    // what fixes that; `vector_store::resident()` only actually takes it on
    // a cache miss, so most searches don't touch it at all — see `vector_store::resident`.
    let t_store_load = Instant::now();
    let store = vector_store::resident(VECTOR_STORE_PATH.as_path())?;
    let store_load_ms = t_store_load.elapsed().as_secs_f64() * 1000.0;

    let t_stage1 = Instant::now();
    let stage1 = store.near_family(
        &query_embed,
        &query_desc.rose,
        &query_gram,
        &query_color,
        limits.stage1_floor,
    );
    let stage1_ms = t_stage1.elapsed().as_secs_f64() * 1000.0;
    log::info!(
        "[search] near family: {} of {} indexed files cleared embed cosine {:.2}",
        stage1.len(),
        store.live_count(),
        limits.stage1_floor,
    );
    // Top hit's full breakdown. rose/gram no longer select anything, so this
    // is the only place their disagreement with the embedding shows up — a
    // high embed_sim next to a low design_sim is the signature of the
    // threshold admitting something the old descriptor would have rejected.
    if let Some(top) = stage1.first() {
        log::debug!(
            "[search] top hit id={} embed_sim={:.4} design_sim={:.4} rose_sim={:.4} gram_sim={:.4}",
            top.id, top.embed_sim, top.design_sim, top.rose_sim, top.gram_sim,
        );
    }

    // Calibration instrumentation only —
    // does not change what's returned. `near_family` only reports entries
    // that already cleared `NEAR_FAMILY_MIN_SIM`; this separately scans the
    // whole population so the log shows what a *lower* floor would have
    // admitted too, which is what the calibration procedure needs. Gated
    // behind `CALIBRATION_LOGGING_ENABLED` because it's a second
    // full-population embedding scan — comparable in cost to the one
    // `near_family` already does, so it isn't something to leave on forever.
    if CALIBRATION_LOGGING_ENABLED {
        let hist = store.cosine_histogram(&query_embed, &CALIBRATION_BUCKETS);
        let live = store.live_count().max(1);
        log::info!(
            "[calibration] cosine histogram: {} pct_at_floor={:.1}%",
            CALIBRATION_BUCKETS.iter().zip(&hist).map(|(t, n)| format!("{t:.2}={n}")).collect::<Vec<_>>().join(" "),
            stage1.len() as f64 / live as f64 * 100.0,
        );
    }

    let t_id_map = Instant::now();
    let con = database::open()?;
    let mut id_map: HashMap<i64, (String, String)> = HashMap::new();
    for folder_str in &scope_paths {
        for (id, path) in database::folder_id_map(&con, folder_str)? {
            id_map.insert(id, (path, folder_str.clone()));
        }
    }
    drop(con);
    let id_map_ms = t_id_map.elapsed().as_secs_f64() * 1000.0;

    let mut seen_paths: HashSet<String> = HashSet::new();
    let query_abs = image_path.to_string_lossy().to_string();
    let mut tier_a: Vec<Candidate> = stage1
        .into_iter()
        .filter_map(|m| {
            let (path, folder) = id_map.get(&m.id)?.clone();
            if path == query_abs || !Path::new(&path).exists() || !seen_paths.insert(path.clone()) {
                return None;
            }
            Some(Candidate { m, path, folder, kind: MatchKind::None, hash_distance: None })
        })
        .collect();

    // `tier_a` is sorted by embed_sim descending (stage1's own order, preserved through the filter above), so a
    // prefix keeps exactly the highest-cosine members — see `VERIFY_MAX_CANDIDATES`. Tier A is the fast tier
    // (cosine at or above the fast floor); the rest is the deep band `[deep_min_sim, NEAR_FAMILY_MIN_SIM)`.
    let sims: Vec<f32> = tier_a.iter().map(|c| c.m.embed_sim).collect();
    let tier_a_n = tier_a_count(&sims);
    let mut deep_band = tier_a.split_off(tier_a_n);

    // ── Stage 0 (pHash) and 0b (engine retrieval): primary computes them here; shadow after the results ship ─────
    let t_stage0 = Instant::now();
    let query_descs = QueryDescriptors {
        embed: query_embed.clone(),
        rose: query_desc.rose.clone(),
        gram: query_gram.clone(),
        color: query_color.clone(),
    };
    let variants_ok = query_phash.len() == 8;
    let mut fingerprints_n = 0usize;
    let mut engine = match_engine::Shortlist { hits: Vec::new(), query_features_ms: 0.0, retrieve_ms: 0.0, vocab_ready: false };
    let mut shortlist_ids: HashSet<i64> = HashSet::new();
    let extras: Vec<Candidate> = if primary {
        engine = match_engine::retrieve_shortlist(&query_sidecar_path, sidecar::Priority::Search);
        shortlist_ids = engine.hits.iter().map(|(id, _)| *id).collect();
        let known: HashMap<i64, vector_store::Match> = tier_a.iter().chain(deep_band.iter()).map(|c| (c.m.id, c.m)).collect();
        let variants: &[u64] = if variants_ok { &query_phash } else { &[] };
        match collect_hits(&store, &scope_paths, variants, &shortlist_ids, &query_descs, &known, &id_map, &query_abs) {
            Ok(s0) => {
                fingerprints_n = s0.fingerprints_n;
                merge_extras(&mut tier_a, &mut deep_band, s0.hits)
            }
            Err(e) => {
                log::warn!("[match_engine] stage 0 failed, continuing without it: {e}");
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };
    let stage0_ms = t_stage0.elapsed().as_secs_f64() * 1000.0;

    // Final order: tier A, fingerprint extras (decided, shown at once), then the "late" rows — shortlist-only extras
    // and the deep band — which join the results only once they verify.
    let (hash_extras, sl_only): (Vec<Candidate>, Vec<Candidate>) = extras.into_iter().partition(|c| c.kind != MatchKind::None);
    let head_n = tier_a_n + hash_extras.len();
    let late_start = head_n + sl_only.len();
    let mut candidates = tier_a;
    candidates.extend(hash_extras);
    candidates.extend(sl_only);
    candidates.extend(deep_band);

    let deep_on = limits.deep_on;
    let near_family_n = tier_a_n;
    // The verify set is the capped tier-A prefix plus every engine-shortlist hit that fell outside it.
    let tier_a_verify_n = near_family_n.min(limits.tier_a_max);
    let verify_paths: Vec<String> = candidates[..late_start]
        .iter()
        .enumerate()
        .filter(|(i, c)| *i < tier_a_verify_n || shortlist_ids.contains(&c.m.id))
        .map(|(_, c)| c.path.clone())
        .collect();
    let verify_pool_n = verify_paths.len();
    // Primary verifies from stored features where it has them; every other mode decodes as before.
    let path_ids: HashMap<String, i64> = candidates.iter().map(|c| (c.path.clone(), c.m.id)).collect();
    let split_by_store = |paths: &[String]| -> (Vec<String>, Vec<String>) {
        if !primary {
            return (Vec::new(), paths.to_vec());
        }
        let has = feature_store::with_store(|s| paths.iter().map(|p| path_ids.get(p).is_some_and(|id| s.contains(*id))).collect::<Vec<bool>>())
            .unwrap_or_default();
        split_by_flags(paths, &has)
    };
    let (feat_paths, fallback_paths) = split_by_store(&verify_paths);

    let deep_available_n = candidates.len() - late_start;
    let deep_pool_n = if deep_on { deep_available_n.min(limits.deep_max) } else { 0 };
    let deep_paths: Vec<String> =
        candidates[late_start..late_start + deep_pool_n].iter().map(|c| c.path.clone()).collect();
    let (deep_feat_paths, deep_fallback_paths) = split_by_store(&deep_paths);
    let feature_verify_n = feat_paths.len() + deep_feat_paths.len();
    let fallback_verify_n = fallback_paths.len() + deep_fallback_paths.len();

    let path_index: HashMap<String, usize> =
        candidates[..head_n].iter().enumerate().map(|(i, c)| (c.path.clone(), i)).collect();
    // Late rows (shortlist-only extras, then the verified part of the deep pool) and where each landed in `results`.
    let late_index: HashMap<String, usize> = (head_n..late_start)
        .chain(late_start..late_start + deep_pool_n)
        .map(|i| (candidates[i].path.clone(), i))
        .collect();
    let mut late_pos: HashMap<String, usize> = HashMap::new();

    // The initial set: every Tier A member plus any fingerprint extras, unverified. This is what lets the
    // UI paint a ranked grid before SIFT/RANSAC has looked at a single candidate. Tier B rows join only once verified.
    let mut results: Vec<SearchResult> =
        candidates[..head_n].iter().map(|c| build_result(c, None, "unchecked")).collect();
    match_tier::assign_ranks(&mut results, rank_cmp);
    // One counter for the whole search: `total` grows only when the mirrored pass triggers.
    let mut total = verify_pool_n + deep_pool_n;
    let mut done_so_far = 0usize;
    on_verify_progress(VerifyProgress { done: 0, total, tier: "near", updated: &results });

    let budget = limits.tier_a_budget;
    let t_verify = Instant::now();
    let straight = verify_sets(&query_sidecar_path, &feat_paths, &fallback_paths, &path_ids, false, budget, &SEARCH_CANCEL, |done, _, _, chunk_out| {
        let mut idx = apply_chunk(&mut results, &path_index, &candidates, chunk_out, false);
        idx.extend(apply_late_chunk(&mut results, &late_index, &mut late_pos, &candidates, chunk_out));
        let updated = ranked_updates(&mut results, &idx, rank_cmp);
        on_verify_progress(VerifyProgress { done, total, tier: "near", updated: &updated });
    });
    let verify_ms = t_verify.elapsed().as_secs_f64() * 1000.0;
    done_so_far += straight.done;
    let mut cancelled = straight.cancelled;

    // ── Stage 2b: mirrored retry ───────────────────────────────────────
    // Neither half of the verify stage can match a reflection: SIFT
    // descriptors are not mirror-invariant, and `estimateAffinePartial2D`
    // fits a transform whose determinant is strictly positive, so it cannot
    // represent one. Flipping the *query* converts the problem back into an
    // ordinary rotation + scale + translation.
    //
    // Only run when the first pass proved nothing AND actually got a clean
    // look at the whole pool — a chunk failure or an exhausted budget
    // already explains why nothing verified, and there is no spare budget
    // for a second full traversal in either case. That puts the entire mirrored cost on searches that
    // currently return no family tier at all — the exact case it exists to
    // rescue — and leaves both successful and already-degraded searches
    // untouched.
    let t_mirror = Instant::now();
    let mirror_attempted = !straight.by_path.values().any(|r| r.matched)
        && !straight.any_chunk_failed
        && !straight.budget_exhausted
        && !straight.cancelled
        && !verify_paths.is_empty();
    let mut mirror_hits = 0usize;
    if mirror_attempted {
        log::info!(
            "[search] no verified matches in {verify_pool_n} verified candidates \
             ({near_family_n} in the family) — retrying mirrored"
        );
        total += verify_pool_n;
        let base = done_so_far;
        let mirrored = verify_sets(&query_sidecar_path, &feat_paths, &fallback_paths, &path_ids, true, budget, &SEARCH_CANCEL, |done, _, _, chunk_out| {
            let mut idx = apply_chunk(&mut results, &path_index, &candidates, chunk_out, true);
            idx.extend(apply_late_chunk(&mut results, &late_index, &mut late_pos, &candidates, chunk_out));
            let updated = ranked_updates(&mut results, &idx, rank_cmp);
            mirror_hits += updated.len();
            on_verify_progress(VerifyProgress { done: base + done, total, tier: "mirrored", updated: &updated });
        });
        done_so_far += mirrored.done;
        cancelled |= mirrored.cancelled;
    }
    let mirror_ms = if mirror_attempted { t_mirror.elapsed().as_secs_f64() * 1000.0 } else { 0.0 };

    // ── Stage 2c: deep tier ────────────────────────────────────────────
    // Straight pass only over the Tier B band; just the rows that verify are emitted and kept.
    let t_deep = Instant::now();
    let mut deep_attempted_n = 0usize;
    let mut deep_failed_n = 0usize;
    let mut deep_stopped = "none";
    if deep_on && deep_pool_n > 0 && !cancelled {
        let base = done_so_far;
        let deep = verify_sets(
            &query_sidecar_path,
            &deep_feat_paths,
            &deep_fallback_paths,
            &path_ids,
            false,
            limits.deep_budget,
            &SEARCH_CANCEL,
            |done, _, _, chunk_out| {
                let idx = apply_late_chunk(&mut results, &late_index, &mut late_pos, &candidates, chunk_out);
                let updated = ranked_updates(&mut results, &idx, rank_cmp);
                on_verify_progress(VerifyProgress { done: base + done, total, tier: "deep", updated: &updated });
            },
        );
        deep_attempted_n = deep.attempted.len();
        deep_failed_n = deep.failed_n;
        cancelled |= deep.cancelled;
        deep_stopped = if deep.cancelled {
            "cancelled"
        } else if deep.budget_exhausted {
            "budget"
        } else if deep_available_n > deep_pool_n {
            "cap"
        } else {
            "none"
        };
    } else if deep_on && cancelled {
        deep_stopped = "cancelled";
    }
    let deep_ms = t_deep.elapsed().as_secs_f64() * 1000.0;
    let deep_verified_n = results.len() - head_n;

    // Primary: exact > transformed > colourway > copy, then the pre-engine order. Otherwise verified matches
    // first, then stage-1 score — `verification` (not this ordering) is what tells the client a row's real state.
    results.sort_by(rank_cmp);
    // Nothing is truncated. Every result here is already a member of the near
    // family — it cleared `NEAR_FAMILY_MIN_SIM` — so the set is defined by
    // similarity, not by rank, and cutting it at `top_k` would put back the
    // arbitrary boundary the threshold exists to remove. The client decides how many rows to paint.
    let t_assemble = Instant::now();
    let verified_n = results.iter().filter(|r| r.verified).count();

    // Calibration instrumentation:
    // the lowest cosine among genuinely verified results, across many real
    // queries, is the empirical floor the procedure asks for — cheap, since
    // it's just a min/max over what was already computed, not another scan.
    if CALIBRATION_LOGGING_ENABLED {
        let verified_cosines = results.iter().filter(|r| r.verified).map(|r| r.similarity / 100.0);
        let (mut hi, mut lo) = (f32::MIN, f32::MAX);
        let mut n = 0usize;
        for c in verified_cosines { hi = hi.max(c); lo = lo.min(c); n += 1; }
        if n > 0 {
            log::info!("[calibration] verified cosine range: highest={hi:.4} lowest={lo:.4} (n={n})");
        }
    }

    for (i, r) in results.iter_mut().enumerate() {
        r.rank = i + 1;
    }
    let assemble_ms = t_assemble.elapsed().as_secs_f64() * 1000.0;

    let exact_n = results.iter().filter(|r| matches!(r.match_kind.as_str(), "exact" | "exactTransformed")).count();
    let colourway_n = results.iter().filter(|r| r.match_kind == "colourway").count();
    let copy_n = results.iter().filter(|r| r.match_kind == "copy").count();
    log::info!(
        "[timing] search TOTAL query={:?} scope_folders={} candidates_indexed={} \
         near_family_n={near_family_n} verify_pool_n={verify_pool_n} verify_attempted_n={} \
         verify_budget_exhausted={} top_k_requested={top_k} verified_n={verified_n} results={} \
         scope_ms={scope_ms:.2} query_prep_ms={prep_ms:.2} \
         describe_ms={describe_ms:.2} store_load_ms={store_load_ms:.2} id_map_ms={id_map_ms:.2} \
         near_family_ms={stage1_ms:.2} verify_ms={verify_ms:.2} mirror_attempted={mirror_attempted} \
         mirror_hits={mirror_hits} mirror_ms={mirror_ms:.2} deep_min_sim={deep_min_sim:.2} \
         deep_pool_n={deep_pool_n} deep_attempted_n={deep_attempted_n} deep_verified_n={deep_verified_n} \
         deep_ms={deep_ms:.2} deep_stopped={deep_stopped} search_cancelled={cancelled} \
         assemble_ms={assemble_ms:.2} total_ms={:.2} full_db={full_db} deep_failed_n={deep_failed_n} \
         engine_mode={mode:?} exact_n={exact_n} colourway_n={colourway_n} copy_n={copy_n} fingerprints_n={fingerprints_n} \
         stage0_ms={stage0_ms:.2} engine_shortlist_n={} engine_retrieve_ms={:.2} query_features_ms={:.2} \
         feature_verify_n={feature_verify_n} fallback_verify_n={fallback_verify_n} vocab_ready={}",
        query_sidecar_path.rsplit(['/', '\\']).next().unwrap_or(&query_sidecar_path),
        scope_paths.len(),
        id_map.len(),
        straight.attempted.len(),
        straight.budget_exhausted,
        results.len(),
        t_total.elapsed().as_secs_f64() * 1000.0,
        engine.hits.len(),
        engine.retrieve_ms,
        engine.query_features_ms,
        engine.vocab_ready,
    );
    log::info!(
        "[rank] top5 {}",
        results
            .iter()
            .take(5)
            .map(|r| format!("({} {} inliers={} sim={:.1})", Path::new(&r.path).file_name().and_then(|n| n.to_str()).unwrap_or(&r.path), r.match_kind, r.match_points, r.similarity))
            .collect::<Vec<_>>()
            .join(" "),
    );

    // Shadow mode: hand back what the post-complete engine run needs; the temp query file stays alive with it.
    let shadow = (mode == MatchEngineMode::Shadow).then(|| ShadowInput {
        _temp: temp_guard,
        store,
        scope_paths: scope_paths.clone(),
        query_path: query_sidecar_path.clone(),
        variants: if variants_ok { query_phash } else { Vec::new() },
        descs: query_descs,
        id_map,
        query_abs,
        legacy_results: results.clone(),
        legacy_verify_paths: verify_paths.iter().take(SHADOW_LEGACY_VERIFY).cloned().collect(),
        legacy_verify_ms: verify_ms,
    });
    Ok(SearchOutput { results, failed_files: Vec::new(), shadow })
}

// Convert a possibly-skewed 4-point quad (candidate pixel coordinates) to
// the axis-aligned `{x,y,w,h}` fraction box the UI draws — SIFT/RANSAC can
// return a slightly non-rectangular quad under real-world lens distortion,
// so this is a bounding box around it, not an exact re-projection.
fn quad_to_fraction_box(quad: &[[f32; 2]; 4], size: (u32, u32)) -> MatchRegion {
    let (w, h) = (size.0.max(1) as f32, size.1.max(1) as f32);
    let xs = quad.iter().map(|p| p[0]);
    let ys = quad.iter().map(|p| p[1]);
    let x0 = xs.clone().fold(f32::INFINITY, f32::min).max(0.0);
    let x1 = xs.fold(f32::NEG_INFINITY, f32::max).min(w);
    let y0 = ys.clone().fold(f32::INFINITY, f32::min).max(0.0);
    let y1 = ys.fold(f32::NEG_INFINITY, f32::max).min(h);
    MatchRegion {
        x: (x0 / w).clamp(0.0, 1.0),
        y: (y0 / h).clamp(0.0, 1.0),
        w: ((x1 - x0) / w).clamp(0.0, 1.0),
        h: ((y1 - y0) / h).clamp(0.0, 1.0),
    }
}

// A dropped `TempQueryFile` deletes the temp file it wraps — RAII cleanup
// for the trimmed-query case below.
struct TempQueryFile(Option<PathBuf>);
impl Drop for TempQueryFile {
    fn drop(&mut self) {
        if let Some(p) = &self.0 {
            let _ = std::fs::remove_file(p);
        }
    }
}

// The sidecar takes a path. If `trim_uniform_border` actually changed the
// image, write it to a temp file and hand back that path (cleaned up when
// the returned guard drops); otherwise reuse the original path untouched.
fn query_path_for_sidecar(original: &Path, trimmed_img: &DynamicImage, was_trimmed: bool) -> Result<(String, TempQueryFile)> {
    if !was_trimmed {
        return Ok((original.to_string_lossy().to_string(), TempQueryFile(None)));
    }
    let temp = std::env::temp_dir().join(format!("pictoria_query_trimmed_{}.png", std::process::id()));
    trimmed_img.save(&temp).map_err(|e| PictoriaError::Fatal(format!("Could not stage trimmed query image: {e}")))?;
    let path_str = temp.to_string_lossy().to_string();
    Ok((path_str, TempQueryFile(Some(temp))))
}

// ── Query preparation ─────────────────────────────────────────────────────

const MAX_TRIM: f32 = 0.25;
const TRIM_TOL: i32 = 12;

// Crop a uniform margin (a scanner bed, a white product-shot background) off
// the reference image so the query describes the design, not the background.
// Returns the image untouched when there's no clear margin, the crop would
// be implausibly aggressive, or every side hits the cap (a genuinely plain
// design, where trimming would just return a smaller piece of the same flat colour).
fn trim_uniform_border(img: DynamicImage) -> DynamicImage {
    let rgb = img.to_rgb8();
    let (w, h) = (rgb.width(), rgb.height());
    if w < 8 || h < 8 {
        return img;
    }

    let corners = [rgb.get_pixel(0, 0), rgb.get_pixel(w - 1, 0), rgb.get_pixel(0, h - 1), rgb.get_pixel(w - 1, h - 1)];
    let mut refc = [0i32; 3];
    for c in 0..3 {
        refc[c] = corners.iter().map(|p| p[c] as i32).sum::<i32>() / 4;
    }
    let matches_ref = |p: &image::Rgb<u8>| (0..3).all(|c| (p[c] as i32 - refc[c]).abs() <= TRIM_TOL);

    let step_x = (w / 64).max(1);
    let step_y = (h / 64).max(1);
    let row_is_margin = |y: u32| (0..w).step_by(step_x as usize).all(|x| matches_ref(rgb.get_pixel(x, y)));
    let col_is_margin = |x: u32| (0..h).step_by(step_y as usize).all(|y| matches_ref(rgb.get_pixel(x, y)));

    let max_y = (h as f32 * MAX_TRIM) as u32;
    let max_x = (w as f32 * MAX_TRIM) as u32;

    let mut top = 0; while top < max_y && row_is_margin(top) { top += 1; }
    let mut bottom = 0; while bottom < max_y && row_is_margin(h - 1 - bottom) { bottom += 1; }
    let mut left = 0; while left < max_x && col_is_margin(left) { left += 1; }
    let mut right = 0; while right < max_x && col_is_margin(w - 1 - right) { right += 1; }

    if top == 0 && bottom == 0 && left == 0 && right == 0 {
        return img;
    }
    if top >= max_y && bottom >= max_y && left >= max_x && right >= max_x {
        return img; // uniform all the way in on every side -> a plain image, not a margin
    }

    let nw = w.saturating_sub(left + right);
    let nh = h.saturating_sub(top + bottom);
    if nw < w / 2 || nh < h / 2 || nw < 8 || nh < 8 {
        return img;
    }

    log::debug!("[search] trimmed query border: {w}x{h} -> {nw}x{nh}");
    img.crop_imm(left, top, nw, nh)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quad_covering_whole_frame_is_not_partial() {
        let quad = [[0.0, 0.0], [180.0, 0.0], [180.0, 360.0], [0.0, 360.0]];
        let region = quad_to_fraction_box(&quad, (180, 360));
        assert!(region.w * region.h > PARTIAL_MAX_AREA_FRACTION);
    }

    #[test]
    fn quad_covering_a_corner_is_partial() {
        let quad = [[0.0, 0.0], [200.0, 0.0], [200.0, 400.0], [0.0, 400.0]];
        let region = quad_to_fraction_box(&quad, (1024, 682));
        assert!(region.w * region.h < PARTIAL_MAX_AREA_FRACTION);
    }

    // A candidate never reached because the budget ran out must read as
    // "unchecked", not "rejected" — `verify_pass` should report it as never
    // attempted, and `execute`'s initial pass already marks it that way.
    #[test]
    fn verify_pass_reports_budget_exhaustion_without_marking_anything_attempted() {
        // No sidecar running in a unit test, so every chunk fails to
        // connect — this exercises the same "never actually verified" path
        // a real budget cutoff does, just via a different cause. Either way
        // `attempted` must stay empty for paths that were never resolved.
        let paths = vec!["a".to_string(), "b".to_string()];
        let result = verify_pass("nonexistent-query.png", &paths, false, VerifyMode::Decode, Some(Duration::from_millis(0)), &AtomicBool::new(false), |_, _, _, _| {});
        assert!(result.budget_exhausted, "zero-duration budget must trip immediately");
        assert!(result.attempted.is_empty());
    }

    // A cancel flag raised before the pass starts must stop it before any chunk is sent.
    #[test]
    fn verify_pass_returns_early_when_cancelled() {
        let paths = vec!["a".to_string(), "b".to_string()];
        let cancel = AtomicBool::new(true);
        let result = verify_pass("nonexistent-query.png", &paths, false, VerifyMode::Decode, Some(Duration::from_secs(60)), &cancel, |_, _, _, _| {});
        assert!(result.cancelled);
        assert!(!result.budget_exhausted);
        assert!(result.attempted.is_empty());
        assert_eq!(result.done, 0);
    }

    // No deadline must not panic or trip the budget, and every path is either sent or counted as failed.
    #[test]
    fn verify_pass_without_a_deadline_reaches_every_chunk() {
        let paths = vec!["a".to_string(), "b".to_string()];
        let result = verify_pass("nonexistent-query.png", &paths, false, VerifyMode::Decode, None, &AtomicBool::new(false), |_, _, _, _| {});
        assert!(!result.budget_exhausted);
        assert_eq!(result.done, paths.len());
        // A live dev sidecar on the port answers instead of failing, so only the bookkeeping is asserted.
        assert_eq!(result.any_chunk_failed, result.failed_n == paths.len());
    }

    // With Full DB off the limits are exactly the configured constants, and the deep tier follows the floor.
    #[test]
    fn verify_limits_without_full_db_match_config() {
        for floor in [0.70, 0.40] {
            let l = verify_limits(floor, false);
            assert_eq!(l.stage1_floor, stage1_floor(floor));
            assert_eq!(l.tier_a_max, VERIFY_MAX_CANDIDATES);
            assert_eq!(l.tier_a_budget, Some(Duration::from_millis(VERIFY_TIME_BUDGET_MS)));
            assert_eq!(l.deep_max, DEEP_VERIFY_MAX_CANDIDATES);
            assert_eq!(l.deep_budget, Some(Duration::from_millis(DEEP_VERIFY_TIME_BUDGET_MS)));
            assert_eq!(l.deep_on, deep_enabled(floor));
        }
    }

    // Full DB lifts every bound and turns the deep tier on, even at a floor that alone means deep off.
    #[test]
    fn verify_limits_with_full_db_are_unbounded() {
        let l = verify_limits(NEAR_FAMILY_MIN_SIM, true);
        assert_eq!(l.stage1_floor, f32::NEG_INFINITY);
        assert_eq!(l.tier_a_max, usize::MAX);
        assert_eq!(l.tier_a_budget, None);
        assert_eq!(l.deep_max, usize::MAX);
        assert_eq!(l.deep_budget, None);
        assert!(l.deep_on);
    }

    // Candidates sorted by cosine split at the fast floor: the band below it is Tier B.
    #[test]
    fn tier_split_puts_candidates_below_the_fast_floor_in_tier_b() {
        let sims = [0.92, 0.80, 0.70, 0.69, 0.55, 0.41];
        assert_eq!(tier_a_count(&sims), 3);
        assert_eq!(tier_a_count(&[0.6, 0.5]), 0);
        assert_eq!(tier_a_count(&[]), 0);
    }

    // A floor at or above the fast floor leaves the search exactly as it was: no deep tier, stage 1 at 0.70.
    #[test]
    fn deep_min_sim_at_or_above_fast_floor_has_no_deep_tier() {
        assert!(!deep_enabled(NEAR_FAMILY_MIN_SIM));
        assert!(!deep_enabled(0.9));
        assert_eq!(stage1_floor(NEAR_FAMILY_MIN_SIM), NEAR_FAMILY_MIN_SIM);
        assert_eq!(stage1_floor(0.9), NEAR_FAMILY_MIN_SIM);
        assert!(deep_enabled(0.40));
        assert_eq!(stage1_floor(0.40), 0.40);
    }

    // Out-of-range, missing and non-finite `deepMinSim` values all resolve to something usable.
    #[test]
    fn deep_min_sim_is_clamped_and_defaulted() {
        assert_eq!(resolve_deep_min_sim(None), DEEP_VERIFY_DEFAULT_MIN_SIM);
        assert_eq!(resolve_deep_min_sim(Some(0.10)), DEEP_VERIFY_MIN_SIM_FLOOR);
        assert_eq!(resolve_deep_min_sim(Some(0.95)), NEAR_FAMILY_MIN_SIM);
        assert_eq!(resolve_deep_min_sim(Some(0.5)), 0.5);
        assert_eq!(resolve_deep_min_sim(Some(f32::NAN)), DEEP_VERIFY_DEFAULT_MIN_SIM);
    }

    fn cand(id: i64, sim: f32, kind: MatchKind, dist: Option<u32>) -> Candidate {
        Candidate {
            m: vector_store::Match { id, embed_sim: sim, design_sim: 0.0, rose_sim: 0.0, gram_sim: 0.0, color_sim: 0.5 },
            path: format!("/lib/{id}.png"),
            folder: "/lib".into(),
            kind,
            hash_distance: dist,
        }
    }

    // A hit in tier A only gains its verdict, a deep-band hit moves out of the band, an unknown hit becomes an extra.
    #[test]
    fn merge_extras_routes_each_hit() {
        let mut tier_a = vec![cand(1, 0.9, MatchKind::None, None), cand(2, 0.8, MatchKind::None, None)];
        let mut deep = vec![cand(3, 0.6, MatchKind::None, None), cand(4, 0.5, MatchKind::None, None)];
        let hits = vec![
            cand(2, 0.8, MatchKind::Exact, Some(1)),
            cand(3, 0.6, MatchKind::Colourway, Some(4)),
            cand(9, 0.2, MatchKind::ExactTransformed, Some(0)),
        ];
        let extras = merge_extras(&mut tier_a, &mut deep, hits);
        assert_eq!(tier_a.len(), 2);
        assert_eq!((tier_a[1].kind, tier_a[1].hash_distance), (MatchKind::Exact, Some(1)));
        assert_eq!(tier_a[0].kind, MatchKind::None);
        assert_eq!(deep.iter().map(|c| c.m.id).collect::<Vec<_>>(), vec![4]);
        let mut ids: Vec<i64> = extras.iter().map(|c| c.m.id).collect();
        ids.sort();
        assert_eq!(ids, vec![3, 9]);
    }

    // The pHash verdict survives verification; a strong verified match becomes a copy only in primary mode.
    #[test]
    fn build_result_carries_the_hash_verdict() {
        let c = cand(7, 0.95, MatchKind::Colourway, Some(3));
        let r = build_result(&c, None, "unchecked");
        assert_eq!((r.match_kind.as_str(), r.hash_distance), ("colourway", Some(3)));
        let plain = build_result(&cand(8, 0.9, MatchKind::None, None), None, "unchecked");
        assert_eq!((plain.match_kind.as_str(), plain.hash_distance), ("none", None));
        let strong = sidecar::VerifyResult { matched: true, inliers: 200, inlier_ratio: 0.9, ..Default::default() };
        let v = build_result(&cand(8, 0.9, MatchKind::None, None), Some(&strong), "verified");
        let expected = if match_engine_mode() == MatchEngineMode::Primary { "copy" } else { "none" };
        assert_eq!(v.match_kind, expected);
    }

    // A late candidate joins the results only when it verifies, once, and is updated in place on a later pass.
    #[test]
    fn apply_late_chunk_adds_only_verified_rows_once() {
        let candidates = vec![cand(1, 0.5, MatchKind::None, None), cand(2, 0.4, MatchKind::None, None)];
        let late_index: HashMap<String, usize> = candidates.iter().enumerate().map(|(i, c)| (c.path.clone(), i)).collect();
        let (mut results, mut pos) = (Vec::new(), HashMap::new());
        let hit = |m: bool, inl: u32| sidecar::VerifyResult { matched: m, inliers: inl, inlier_ratio: 0.9, ..Default::default() };
        let mut out = HashMap::new();
        out.insert("/lib/1.png".to_string(), hit(true, 30));
        out.insert("/lib/2.png".to_string(), hit(false, 0));
        assert_eq!(apply_late_chunk(&mut results, &late_index, &mut pos, &candidates, &out).len(), 1);
        assert_eq!(results.len(), 1);
        out.insert("/lib/1.png".to_string(), hit(true, 80));
        apply_late_chunk(&mut results, &late_index, &mut pos, &candidates, &out);
        assert_eq!((results.len(), results[0].match_points), (1, 80));
    }

    // Paths split by their flag keep their order; a missing flag counts as "no features".
    #[test]
    fn split_by_flags_partitions_in_order() {
        let paths: Vec<String> = ["a", "b", "c", "d"].iter().map(|s| s.to_string()).collect();
        let (yes, no) = split_by_flags(&paths, &[true, false, true]);
        assert_eq!(yes, vec!["a", "c"]);
        assert_eq!(no, vec!["b", "d"]);
        let (yes, no) = split_by_flags(&paths, &[]);
        assert!(yes.is_empty() && no.len() == 4);
    }

    // Two passes over disjoint sets fold into one outcome: results unioned, counts summed, flags OR-ed.
    #[test]
    fn merge_passes_combines_outcomes() {
        let mk = |p: &str, done: usize, failed: bool| {
            let mut by_path = HashMap::new();
            by_path.insert(p.to_string(), sidecar::VerifyResult::default());
            VerifyPassResult {
                by_path,
                attempted: [p.to_string()].into_iter().collect(),
                any_chunk_failed: failed,
                budget_exhausted: false,
                cancelled: false,
                done,
                failed_n: if failed { done } else { 0 },
            }
        };
        let m = merge_passes(mk("a", 3, false), mk("b", 2, true));
        assert_eq!((m.by_path.len(), m.attempted.len(), m.done, m.failed_n), (2, 2, 5, 2));
        assert!(m.any_chunk_failed && !m.cancelled);
    }

    // With nothing to verify from features, `verify_sets` still reports every fallback path through one decode pass.
    #[test]
    fn verify_sets_runs_the_fallback_group() {
        let paths = vec!["a".to_string(), "b".to_string()];
        let r = verify_sets("nonexistent.png", &[], &paths, &HashMap::new(), false, None, &AtomicBool::new(false), |_, _, _, _| {});
        assert_eq!(r.done, 2);
        assert!(!r.cancelled && !r.budget_exhausted);
    }

    // End to end against a live sidecar (run alone with `-- --ignored`): the shadow stages on a rotated copy of img_007
    // must find it by pHash and by engine retrieval, verify it from stored features, and report it as engine-only.
    // Needs PICTORIA_E2E_LIB and PICTORIA_E2E_QUERIES (see the match_engine e2e test).
    #[test]
    #[ignore]
    fn e2e_shadow_run_reports_engine_only_copy() {
        use crate::config::{MATCH_VOCAB_ITERS, MATCH_VOCAB_SAMPLE};
        let home = std::env::temp_dir().join(format!("pictoria_e2e_shadow_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        std::env::set_var("HOME", &home);
        let lib = std::env::var("PICTORIA_E2E_LIB").expect("PICTORIA_E2E_LIB");
        let queries = std::env::var("PICTORIA_E2E_QUERIES").expect("PICTORIA_E2E_QUERIES");
        let mut files: Vec<String> =
            std::fs::read_dir(&lib).unwrap().map(|e| e.unwrap().path().to_string_lossy().to_string()).collect();
        files.sort();
        let target = files.iter().position(|p| p.ends_with("img_007.png")).unwrap();

        // Library rows, fingerprints, features, vectors.
        let con = database::open().unwrap();
        let mut store = vector_store::VectorStore::new();
        let mut id_map = HashMap::new();
        let unit = |seed: f32, n: usize| -> Vec<f32> {
            let v: Vec<f32> = (0..n).map(|i| ((i as f32 + 1.0) * seed).sin()).collect();
            let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            v.into_iter().map(|x| x / norm).collect()
        };
        let embed_total = crate::config::EMBED_DIM * crate::config::EMBED_ZOOM_LEVELS;
        let gram_total = crate::config::GRAM_ZOOM_LEVELS * crate::config::GRAM_DIM_PER_ZOOM;
        for (i, path) in files.iter().enumerate() {
            database::insert_file(&con, path, &format!("h{i}"), i as i64, 0.0).unwrap();
            let fp = sidecar::fingerprint(std::slice::from_ref(path), sidecar::Priority::Index).unwrap();
            let file_id = database::file_id_by_path(&con, path).unwrap().unwrap();
            database::upsert_fingerprint(&con, file_id, fp[0].1.as_ref().unwrap().first().copied()).unwrap();
            let f = sidecar::extract_features(std::slice::from_ref(path), false, sidecar::Priority::Index).unwrap().remove(0).1.unwrap();
            match_engine::index_file_features(i as i64, Some(&f), None);
            store
                .upsert(i as i64, &unit(0.1 + i as f32, embed_total), &unit(0.2, crate::config::ROSE_DIM), &unit(0.3, gram_total), &unit(0.4, crate::core::color::COLOR_DIM))
                .unwrap();
            id_map.insert(i as i64, (path.clone(), lib.clone()));
        }
        let sample = feature_store::with_store(|s| s.sample_descriptors(MATCH_VOCAB_SAMPLE)).unwrap();
        let vocab = sidecar::train_vocab(&sample, crate::config::MATCH_VOCAB_K, MATCH_VOCAB_ITERS).unwrap();
        feature_store::with_store(|s| s.save_vocab(&vocab)).unwrap().unwrap();
        match_engine::assign_words();

        let query = format!("{queries}/rot90.png");
        let variants = sidecar::fingerprint(&[query.clone()], sidecar::Priority::Index).unwrap().remove(0).1.unwrap();
        assert_eq!(variants.len(), 8);
        let row = |p: &str, verified: bool| SearchResult {
            rank: 1,
            path: p.to_string(),
            name: p.to_string(),
            similarity: 50.0,
            pattern_match: 50.0,
            color_match: 0.0,
            folder: lib.clone(),
            verified,
            verification: if verified { "verified" } else { "unchecked" }.into(),
            partial: false,
            match_region: MatchRegion { x: 0.0, y: 0.0, w: 1.0, h: 1.0 },
            match_points: 0,
            mirrored: false,
            match_kind: "none".into(),
            hash_distance: None,
        };
        let input = ShadowInput {
            _temp: TempQueryFile(None),
            store: Arc::new(store),
            scope_paths: vec![lib.clone()],
            query_path: query.clone(),
            variants,
            descs: QueryDescriptors { embed: unit(0.5, embed_total), rose: unit(0.2, crate::config::ROSE_DIM), gram: unit(0.3, gram_total), color: unit(0.4, crate::core::color::COLOR_DIM) },
            id_map,
            query_abs: query,
            // Legacy missed the copy: only an unrelated file counts as verified.
            legacy_results: vec![row(&files[3], true), row(&files[target], false)],
            legacy_verify_paths: vec![files[3].clone()],
            legacy_verify_ms: 1.0,
        };
        let r = shadow_report(&input).expect("shadow report");
        println!("shadow report: {r:?}");
        assert_eq!(r.exact_n, 1, "the rotated copy is one exact (transformed) hit by pHash");
        assert!(r.vocab_ready && r.shortlist_n > 0);
        assert!(r.engine_only.iter().any(|n| n == "img_007.png"), "engine_only should name img_007: {:?}", r.engine_only);
        assert!(r.copy_n >= 1 && r.fallback_n == 0);
    }
}
