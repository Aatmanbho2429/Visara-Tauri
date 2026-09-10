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
// rejected — see SEARCH-LATENCY-PLAN.md Phase 1.
//
// Results stream to the caller as they're known: an all-`"unchecked"` set the
// moment stage 1 finishes, then per-chunk updates as verification proves or
// rejects each candidate — see `VerifyProgress` and Phase 4 of the same plan.
//
// Multi-folder model unchanged: empty `scope` means every watched folder.

use crate::{
    config::{CALIBRATION_LOGGING_ENABLED, NEAR_FAMILY_MIN_SIM, VECTOR_STORE_PATH, VERIFY_MAX_CANDIDATES, VERIFY_TIME_BUDGET_MS},
    core::{database, search_gate, sidecar, vector_store},
    error::{Result, PictoriaError},
    models::response::{FailedFile, MatchRegion, SearchResult},
    utils::image_loader,
};
use image::DynamicImage;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

// ── Public API ────────────────────────────────────────────────────────────

// The verify pool is no longer a fixed shortlist. `VectorStore::near_family`
// returns every file whose DINO embedding cosine clears
// `config::NEAR_FAMILY_MIN_SIM` — membership is decided by similarity rather
// than a rank cut. What IS bounded is how much of that family gets
// SIFT/RANSAC-verified in one search: `VERIFY_MAX_CANDIDATES` caps the count
// and `VERIFY_TIME_BUDGET_MS` caps the wall clock, taken from the
// highest-cosine prefix since `candidates` is already sorted that way. See
// SEARCH-LATENCY-PLAN.md Phase 1 for why an unbounded verify pass is the
// entire reason a search could take 79s.
//
// The old fixed top-1500 rank cut this replaced had the property that a real
// match could be excluded purely because a library held more than 1500 tiles
// closer to the query — invisible from the outside, and unfixable by the
// user. `near_family_n` in the timing log below is the number to watch if a
// search ever feels slow; a large fraction of the library clearing the floor
// means the floor, not the verify budget, is what needs recalibrating (see
// `VectorStore::cosine_histogram` and Phase 3).

// Verify runs in chunks of this size rather than one request for the whole
// shortlist, purely so `on_verify_progress` below has something to report
// between chunks — SIFT/RANSAC on a 200-candidate shortlist takes tens of
// seconds even parallelized server-side (see `sidecar/pipeline.py`), and a
// single request that only resolves at the very end left the UI's progress
// bar sitting frozen at 0% for the whole wait, indistinguishable from hung.
//
// Raised from 20 to 100 (SEARCH-LATENCY-PLAN.md Phase 2b): the sidecar
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

// Cosine thresholds the calibration log (Phase 3) reports population counts
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
}

// Reported after every verify chunk (including a zero-progress call before
// the first one starts). `updated` carries the `SearchResult`s that changed
// in this tick — freshly verified or rejected — so the caller can stream
// them on rather than wait for the whole search to finish; it's empty for
// the very first, all-`"unchecked"` call. `mirrored` distinguishes the
// second, flipped-query pass so the UI can label it rather than have the
// counters look like they restarted.
pub struct VerifyProgress<'a> {
    pub done: usize,
    pub total: usize,
    pub mirrored: bool,
    pub updated: &'a [SearchResult],
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
    budget: Duration,
    mut on_chunk: impl FnMut(usize, usize, bool, &HashMap<String, sidecar::VerifyResult>),
) -> VerifyPassResult {
    let total = paths.len();
    let mut out = HashMap::with_capacity(total);
    let mut attempted = HashSet::with_capacity(total);
    let mut any_chunk_failed = false;
    let mut budget_exhausted = false;
    let mut done = 0usize;
    let deadline = Instant::now() + budget;

    on_chunk(0, total, mirror, &HashMap::new());
    for chunk in paths.chunks(VERIFY_CHUNK) {
        if Instant::now() >= deadline {
            budget_exhausted = true;
            log::warn!(
                "[search] verify budget ({budget:?}) exhausted after {done}/{total} \
                 candidates (mirror={mirror}) — remainder left unchecked, not rejected"
            );
            break;
        }
        let mut chunk_out: HashMap<String, sidecar::VerifyResult> = HashMap::with_capacity(chunk.len());
        match sidecar::verify(query_path, chunk, mirror, sidecar::Priority::Search) {
            Ok(results) => {
                for (path, result) in results {
                    attempted.insert(path.clone());
                    chunk_out.insert(path, result);
                }
            }
            Err(e) => {
                any_chunk_failed = true;
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

    VerifyPassResult { by_path: out, attempted, any_chunk_failed, budget_exhausted }
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
    }
}

// Apply one verify chunk's results onto `results` in place, returning just
// the rows that changed so the caller can stream that slice on without
// re-sending everything. `mirror_hits_only`, set for the mirrored pass,
// keeps a straight-pass miss's data intact — a mirrored miss carries no more
// information than the straight miss already recorded, and overwriting
// would discard the first pass's inlier counts for no reason.
fn apply_chunk(
    results: &mut [SearchResult],
    path_index: &HashMap<String, usize>,
    candidates: &[Candidate],
    chunk_out: &HashMap<String, sidecar::VerifyResult>,
    mirror_hits_only: bool,
) -> Vec<SearchResult> {
    let mut updated = Vec::with_capacity(chunk_out.len());
    for (path, v) in chunk_out {
        if mirror_hits_only && !v.matched {
            continue;
        }
        if let Some(&i) = path_index.get(path) {
            let verification = if v.matched { "verified" } else { "rejected" };
            results[i] = build_result(&candidates[i], Some(v), verification);
            updated.push(results[i].clone());
        }
    }
    updated
}

// `on_verify_progress` fires once before verification starts (the initial
// all-`"unchecked"` set) and once per verify chunk after — see
// `VerifyProgress`.
pub fn execute(
    image_path: &Path,
    scope: &[PathBuf],
    // What the client asked to display. No longer trims the result set — every
    // near-family member is returned and the client paginates — but it is still
    // logged, because a large gap between it and `results` is the first thing
    // to look at if the UI feels heavy after a broad search.
    top_k: usize,
    mut on_verify_progress: impl FnMut(VerifyProgress),
) -> Result<(Vec<SearchResult>, Vec<FailedFile>)> {
    let t_total = Instant::now();

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
    let (query_sidecar_path, _temp_guard) = query_path_for_sidecar(image_path, &query_img, trimmed)?;
    let prep_ms = t_prep.elapsed().as_secs_f64() * 1000.0;

    let t_describe = Instant::now();
    let described = sidecar::describe(&[query_sidecar_path.clone()], sidecar::Priority::Search)?;
    let describe_ms = t_describe.elapsed().as_secs_f64() * 1000.0;
    let query_desc = described
        .into_iter()
        .next()
        .and_then(|(_, d)| d)
        .ok_or_else(|| PictoriaError::Fatal("Could not describe the reference image.".into()))?;
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
    // the read guard must NOT (SEARCH-LATENCY-PLAN.md Phase 1d) — holding it
    // across the whole, multi-second verify phase below blocks the indexer's
    // `store.save()` in `core::watcher` for that entire window. Scoping the
    // guard to just this block, rather than the rest of the function, is
    // what fixes that; `vector_store::resident()` only actually takes it on
    // a cache miss, so most searches don't touch it at all — see Phase 5a.
    let t_store_load = Instant::now();
    let store = vector_store::resident(VECTOR_STORE_PATH.as_path())?;
    let store_load_ms = t_store_load.elapsed().as_secs_f64() * 1000.0;

    let t_stage1 = Instant::now();
    let stage1 = store.near_family(
        &query_embed,
        &query_desc.rose,
        &query_gram,
        &query_color,
        NEAR_FAMILY_MIN_SIM,
    );
    let stage1_ms = t_stage1.elapsed().as_secs_f64() * 1000.0;
    log::info!(
        "[search] near family: {} of {} indexed files cleared embed cosine {:.2}",
        stage1.len(),
        store.live_count(),
        NEAR_FAMILY_MIN_SIM,
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

    // Calibration instrumentation only (SEARCH-LATENCY-PLAN.md Phase 3) —
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
    let candidates: Vec<Candidate> = stage1
        .into_iter()
        .filter_map(|m| {
            let (path, folder) = id_map.get(&m.id)?.clone();
            if path == query_abs || !Path::new(&path).exists() || !seen_paths.insert(path.clone()) {
                return None;
            }
            Some(Candidate { m, path, folder })
        })
        .collect();

    // ── Stage 2: geometrically verify a bounded, cosine-ordered prefix ──
    // `candidates` is already sorted by embed_sim descending (stage1's own
    // order, preserved through the filter above), so taking a prefix here
    // keeps exactly the highest-cosine members — see `VERIFY_MAX_CANDIDATES`.
    let near_family_n = candidates.len();
    let verify_pool_n = near_family_n.min(VERIFY_MAX_CANDIDATES);
    let verify_paths: Vec<String> = candidates[..verify_pool_n].iter().map(|c| c.path.clone()).collect();

    let path_index: HashMap<String, usize> =
        candidates.iter().enumerate().map(|(i, c)| (c.path.clone(), i)).collect();

    // The initial set: every family member, unverified. This is what lets the
    // UI paint a ranked grid before SIFT/RANSAC has looked at a single
    // candidate — see SEARCH-LATENCY-PLAN.md Phase 4.
    let mut results: Vec<SearchResult> =
        candidates.iter().map(|c| build_result(c, None, "unchecked")).collect();
    on_verify_progress(VerifyProgress { done: 0, total: verify_pool_n, mirrored: false, updated: &results });

    let budget = Duration::from_millis(VERIFY_TIME_BUDGET_MS);
    let t_verify = Instant::now();
    let straight = verify_pass(&query_sidecar_path, &verify_paths, false, budget, |done, total, mirrored, chunk_out| {
        let updated = apply_chunk(&mut results, &path_index, &candidates, chunk_out, false);
        on_verify_progress(VerifyProgress { done, total, mirrored, updated: &updated });
    });
    let verify_ms = t_verify.elapsed().as_secs_f64() * 1000.0;

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
    // for a second full traversal in either case (SEARCH-LATENCY-PLAN.md
    // Phase 1c). That puts the entire mirrored cost on searches that
    // currently return no family tier at all — the exact case it exists to
    // rescue — and leaves both successful and already-degraded searches
    // untouched.
    let t_mirror = Instant::now();
    let mirror_attempted = !straight.by_path.values().any(|r| r.matched)
        && !straight.any_chunk_failed
        && !straight.budget_exhausted
        && !verify_paths.is_empty();
    let mut mirror_hits = 0usize;
    if mirror_attempted {
        log::info!(
            "[search] no verified matches in {verify_pool_n} verified candidates \
             ({near_family_n} in the family) — retrying mirrored"
        );
        let mirrored = verify_pass(&query_sidecar_path, &verify_paths, true, budget, |done, total, m, chunk_out| {
            let updated = apply_chunk(&mut results, &path_index, &candidates, chunk_out, true);
            mirror_hits += updated.len();
            on_verify_progress(VerifyProgress { done, total, mirrored: m, updated: &updated });
        });
        let _ = mirrored; // by_path/attempted not needed past the chunk-apply above
    }
    let mirror_ms = if mirror_attempted { t_mirror.elapsed().as_secs_f64() * 1000.0 } else { 0.0 };

    // Verified matches first (the stage-1 score breaks ties within that
    // group, same as every other tier), then rejected/unchecked together by
    // stage-1 score — `verification` (not this ordering) is what tells the
    // client which of those two a given row actually is.
    results.sort_by(|a, b| {
        b.verified.cmp(&a.verified).then_with(|| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal))
    });
    // Nothing is truncated. Every result here is already a member of the near
    // family — it cleared `NEAR_FAMILY_MIN_SIM` — so the set is defined by
    // similarity, not by rank, and cutting it at `top_k` would put back the
    // arbitrary boundary the threshold exists to remove. The verified tier
    // leads (geometric proof), the rest follow, and the client decides how
    // many of them to paint.
    let t_assemble = Instant::now();
    let verified_n = results.iter().take_while(|r| r.verified).count();

    // Calibration instrumentation (SEARCH-LATENCY-PLAN.md Phase 3, part 2):
    // the lowest cosine among genuinely verified results, across many real
    // queries, is the empirical floor the procedure asks for — cheap, since
    // it's just a min/max over what was already computed, not another scan.
    if CALIBRATION_LOGGING_ENABLED {
        let verified_cosines = results.iter().take(verified_n).map(|r| r.similarity / 100.0);
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

    log::info!(
        "[timing] search TOTAL query={:?} scope_folders={} candidates_indexed={} \
         near_family_n={near_family_n} verify_pool_n={verify_pool_n} verify_attempted_n={} \
         verify_budget_exhausted={} top_k_requested={top_k} verified_n={verified_n} results={} \
         scope_ms={scope_ms:.2} query_prep_ms={prep_ms:.2} \
         describe_ms={describe_ms:.2} store_load_ms={store_load_ms:.2} id_map_ms={id_map_ms:.2} \
         near_family_ms={stage1_ms:.2} verify_ms={verify_ms:.2} mirror_attempted={mirror_attempted} \
         mirror_hits={mirror_hits} mirror_ms={mirror_ms:.2} assemble_ms={assemble_ms:.2} \
         total_ms={:.2}",
        query_sidecar_path.rsplit(['/', '\\']).next().unwrap_or(&query_sidecar_path),
        scope_paths.len(),
        id_map.len(),
        straight.attempted.len(),
        straight.budget_exhausted,
        results.len(),
        t_total.elapsed().as_secs_f64() * 1000.0,
    );

    Ok((results, Vec::new()))
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
        let result = verify_pass("nonexistent-query.png", &paths, false, Duration::from_millis(0), |_, _, _, _| {});
        assert!(result.budget_exhausted, "zero-duration budget must trip immediately");
        assert!(result.attempted.is_empty());
    }
}
