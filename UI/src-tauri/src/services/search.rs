//! Search execution — describes the query via the sidecar, ranks every file
//! in scope against it (stage 1, in-process), then geometrically verifies the
//! shortlist through the sidecar (stage 2) before returning results.
//!
//! Multi-folder model unchanged: empty `scope` means every watched folder.

use crate::{
    config::VECTOR_STORE_PATH,
    core::{database, search_gate, sidecar, vector_store::VectorStore},
    error::{Result, PictoriaError},
    utils::image_loader,
};
use image::DynamicImage;
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    time::Instant,
};

// ── Result types ──────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone, Copy)]
pub struct MatchRegion {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Serialize, Clone)]
pub struct SearchResult {
    pub rank: usize,
    pub path: String,
    pub name: String,
    /// Stage-1 combined score (Gabor rose + Gram-matrix), scaled 0-100.
    pub similarity: f32,
    pub pattern_match: f32,
    pub color_match: f32,
    pub folder: String,
    /// True once SIFT/RANSAC has actually proven the query sits inside this
    /// file — a direct geometric fact, not a similarity threshold.
    pub verified: bool,
    /// True when `verified` AND the matched region is a small piece of this
    /// file rather than nearly the whole frame — i.e. genuinely "found
    /// inside a bigger design", not just "this is basically the same image".
    pub partial: bool,
    pub match_region: MatchRegion,
    /// SIFT inlier count backing `verified` — the UI's confidence badge
    /// (e.g. "embedded · 103 pts"). Zero when not verified.
    pub match_points: u32,
    /// True when this was only provable against a horizontally-flipped
    /// query — a book-matched or mirrored copy of the reference rather than
    /// a straight one. Always false unless `verified`.
    pub mirrored: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct FailedFile {
    pub file: String,
    pub reason: String,
}

// ── Public API ────────────────────────────────────────────────────────────

/// How many stage-1 candidates get geometrically verified — fixed,
/// independent of `top_k` (the display picker). This used to scale with
/// `top_k` (`top_k * 4`, min 40), which meant asking for more results
/// changed *which* candidates got SIFT-verified, not just how many got
/// shown: a "found inside" hit outside the smaller shortlist could appear
/// only once the picker grew, reshuffling the whole ranking rather than
/// extending it. A fixed pool means the verified ("family") tier is a
/// stable prefix no matter what `top_k` is set to.
const VERIFY_SHORTLIST_FIXED: usize = 1500;

/// Verify runs in chunks of this size rather than one request for the whole
/// shortlist, purely so `on_verify_progress` below has something to report
/// between chunks — SIFT/RANSAC on a 200-candidate shortlist takes tens of
/// seconds even parallelized server-side (see `sidecar/pipeline.py`), and a
/// single request that only resolves at the very end left the UI's progress
/// bar sitting frozen at 0% for the whole wait, indistinguishable from hung.
const VERIFY_CHUNK: usize = 20;

/// A verified match is "found inside" rather than "same image" when its
/// matched region covers less than this fraction of the candidate's area.
const PARTIAL_MAX_AREA_FRACTION: f32 = 0.85;

/// Run the verify shortlist through the sidecar, chunk by chunk, reporting
/// progress between chunks. `mirror` flips the query horizontally — the only
/// way a mirrored match can be found (see `sidecar::verify`).
///
/// A chunk that fails degrades those candidates to stage-1-only ranking
/// rather than failing the whole search, but it is *logged* rather than
/// silently swallowed: the mirrored retry below triggers on "nothing
/// verified", and a transport failure produces exactly that state while
/// meaning something completely different.
fn verify_pass(
    query_path: &str,
    paths: &[String],
    mirror: bool,
    on_progress: &mut impl FnMut(usize, usize, bool),
) -> HashMap<String, sidecar::VerifyResult> {
    let total = paths.len();
    let mut out = HashMap::with_capacity(total);
    let mut done = 0usize;
    on_progress(0, total, mirror);
    for chunk in paths.chunks(VERIFY_CHUNK) {
        match sidecar::verify(query_path, chunk, mirror, sidecar::Priority::Search) {
            Ok(results) => out.extend(results),
            Err(e) => log::warn!(
                "[search] verify chunk failed (n={}, mirror={mirror}): {e} — \
                 those candidates fall back to stage-1 ranking only",
                chunk.len(),
            ),
        }
        done += chunk.len();
        on_progress(done, total, mirror);
    }
    out
}

/// `on_verify_progress` is `(done, total, mirrored_pass)` — the third
/// argument lets the caller label the mirrored retry distinctly, so the bar
/// restarting from zero reads as a second phase rather than a glitch.
pub fn execute(
    image_path: &Path,
    scope: &[PathBuf],
    top_k: usize,
    mut on_verify_progress: impl FnMut(usize, usize, bool),
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
    let query_gram: Vec<f32> = query_desc.gram.into_iter().flatten().collect();
    // Colour histogram is computed sidecar-side now (same decode as rose/gram —
    // see pipeline.color_histogram), not a separate Rust-side pass over
    // `query_img`. `query_sidecar_path` points at the same (possibly trimmed)
    // image `query_img` represents, so this is the same content either way.
    let query_color = query_desc.color;

    // ── Stage 1: rank every file in scope ──────────────────────────────
    let t_store_load = Instant::now();
    let _store_guard = crate::core::vector_store::store_io_read_guard();
    let store = VectorStore::load(VECTOR_STORE_PATH.as_path())?;
    let store_load_ms = t_store_load.elapsed().as_secs_f64() * 1000.0;

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

    let shortlist_n = VERIFY_SHORTLIST_FIXED;
    let t_stage1 = Instant::now();
    let stage1 = store.search(&query_desc.rose, &query_gram, &query_color, shortlist_n);
    let stage1_ms = t_stage1.elapsed().as_secs_f64() * 1000.0;

    let mut seen_paths: HashSet<String> = HashSet::new();
    let query_abs = image_path.to_string_lossy().to_string();
    struct Candidate { m: crate::core::vector_store::Match, path: String, folder: String }
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

    // ── Stage 2: geometrically verify the shortlist, chunk by chunk ────
    // Chunked (not one request for all of it) so `on_verify_progress` can
    // report real progress between chunks instead of the caller sitting
    // blind until the very end — see `VERIFY_CHUNK`'s doc comment.
    let shortlist_paths: Vec<String> = candidates.iter().map(|c| c.path.clone()).collect();
    let n_shortlist = shortlist_paths.len();
    let t_verify = Instant::now();
    let mut verify_by_path =
        verify_pass(&query_sidecar_path, &shortlist_paths, false, &mut on_verify_progress);
    let verify_ms = t_verify.elapsed().as_secs_f64() * 1000.0;

    // ── Stage 2b: mirrored retry ───────────────────────────────────────
    // Neither half of the verify stage can match a reflection: SIFT
    // descriptors are not mirror-invariant, and `estimateAffinePartial2D`
    // fits a transform whose determinant is strictly positive, so it cannot
    // represent one. Flipping the *query* converts the problem back into an
    // ordinary rotation + scale + translation.
    //
    // Only run when the first pass proved nothing, because it costs a second
    // full traversal of the shortlist. That puts the entire cost on searches
    // that currently return no family tier at all — the exact case this
    // exists to rescue — and leaves successful searches untouched.
    let t_mirror = Instant::now();
    let mirror_attempted = !verify_by_path.values().any(|r| r.matched) && !shortlist_paths.is_empty();
    let mut mirror_hits = 0usize;
    if mirror_attempted {
        log::info!(
            "[search] no verified matches in {n_shortlist} candidates — retrying mirrored"
        );
        let mirrored =
            verify_pass(&query_sidecar_path, &shortlist_paths, true, &mut on_verify_progress);
        // Keep only the wins. A mirrored miss carries no more information
        // than the straight miss already recorded, and overwriting would
        // discard the first pass's inlier counts for no reason.
        for (path, result) in mirrored {
            if result.matched {
                verify_by_path.insert(path, result);
                mirror_hits += 1;
            }
        }
    }
    let mirror_ms = if mirror_attempted { t_mirror.elapsed().as_secs_f64() * 1000.0 } else { 0.0 };

    let scale = |v: f32| (v.clamp(0.0, 1.0) * 100.0 * 10.0).round() / 10.0;

    let t_assemble = Instant::now();
    let mut results: Vec<SearchResult> = candidates
        .into_iter()
        .map(|c| {
            let name = Path::new(&c.path).file_name().and_then(|n| n.to_str()).unwrap_or(&c.path).to_string();
            let v = verify_by_path.get(&c.path);
            let verified = v.map(|r| r.matched).unwrap_or(false);

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
                path: c.path,
                name,
                similarity: scale(c.m.score),
                pattern_match: scale(c.m.score),
                color_match: scale(c.m.color_sim),
                folder: c.folder,
                verified,
                partial,
                match_region,
                match_points: if verified { v.map(|r| r.inliers).unwrap_or(0) } else { 0 },
                mirrored: verified && v.map(|r| r.mirrored).unwrap_or(false),
            }
        })
        .collect();

    // Verified matches first (strongest inlier count within that group via
    // the stage-1 score as a tiebreaker), then everything else by stage-1 score.
    results.sort_by(|a, b| {
        b.verified.cmp(&a.verified).then_with(|| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal))
    });
    // Verified results are a geometric proof, not a similarity guess — keep
    // every one of them (the "family" tier) rather than letting `top_k` cut
    // a real match off. `top_k` instead caps only the unverified "similar"
    // tail that follows. Since the sort above put every verified result
    // first, `family_count` is just the length of that prefix.
    let family_count = results.iter().take_while(|r| r.verified).count();
    results.truncate(family_count + top_k);
    for (i, r) in results.iter_mut().enumerate() {
        r.rank = i + 1;
    }
    let assemble_ms = t_assemble.elapsed().as_secs_f64() * 1000.0;

    log::info!(
        "[timing] search TOTAL query={:?} scope_folders={} candidates_indexed={} \
         shortlist={n_shortlist} results={} scope_ms={scope_ms:.2} query_prep_ms={prep_ms:.2} \
         describe_ms={describe_ms:.2} store_load_ms={store_load_ms:.2} id_map_ms={id_map_ms:.2} \
         stage1_rank_ms={stage1_ms:.2} verify_ms={verify_ms:.2} mirror_attempted={mirror_attempted} \
         mirror_hits={mirror_hits} mirror_ms={mirror_ms:.2} assemble_ms={assemble_ms:.2} \
         total_ms={:.2}",
        query_sidecar_path.rsplit(['/', '\\']).next().unwrap_or(&query_sidecar_path),
        scope_paths.len(),
        id_map.len(),
        results.len(),
        t_total.elapsed().as_secs_f64() * 1000.0,
    );

    Ok((results, Vec::new()))
}

/// Convert a possibly-skewed 4-point quad (candidate pixel coordinates) to
/// the axis-aligned `{x,y,w,h}` fraction box the UI draws — SIFT/RANSAC can
/// return a slightly non-rectangular quad under real-world lens distortion,
/// so this is a bounding box around it, not an exact re-projection.
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

/// A dropped `TempQueryFile` deletes the temp file it wraps — RAII cleanup
/// for the trimmed-query case below.
struct TempQueryFile(Option<PathBuf>);
impl Drop for TempQueryFile {
    fn drop(&mut self) {
        if let Some(p) = &self.0 {
            let _ = std::fs::remove_file(p);
        }
    }
}

/// The sidecar takes a path. If `trim_uniform_border` actually changed the
/// image, write it to a temp file and hand back that path (cleaned up when
/// the returned guard drops); otherwise reuse the original path untouched.
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

/// Crop a uniform margin (a scanner bed, a white product-shot background) off
/// the reference image so the query describes the design, not the background.
/// Returns the image untouched when there's no clear margin, the crop would
/// be implausibly aggressive, or every side hits the cap (a genuinely plain
/// design, where trimming would just return a smaller piece of the same flat colour).
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
}
