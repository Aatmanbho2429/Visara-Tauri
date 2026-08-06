//! Search execution — embeds the query image, queries the vector store across
//! one or more watched folders, and maps IDs back to file paths.
//!
//! Multi-folder model: callers pass a list of folder paths (the scope).  If
//! the list is empty, the scope is automatically every watched folder in the
//! Library.  The watcher keeps each folder's index current, so this function
//! no longer triggers an on-the-fly sync — it just searches the store.

use crate::{
    config::VECTOR_STORE_PATH,
    core::{database, embedder, vector_store::VectorStore},
    error::{Result, PictoriaError},
    utils::image_loader,
};
use image::DynamicImage;
use serde::Serialize;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

// ── Result types ──────────────────────────────────────────────────────────

/// Where in a result image the query was found, as fractions of its width and
/// height, so the UI can draw the box over a thumbnail of any size.
#[derive(Debug, Serialize, Clone, Copy)]
pub struct MatchRegion {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Serialize, Clone)]
pub struct SearchResult {
    pub rank:          usize,
    pub path:          String,
    pub name:          String,
    /// Blended score (pattern × 0.85 + color × 0.15), scaled 0–100.
    pub similarity:    f32,
    /// Design/pattern cosine similarity, scaled 0–100.  Primary match signal.
    pub pattern_match: f32,
    /// Colour histogram cosine similarity, scaled 0–100.  Secondary signal.
    pub color_match:   f32,
    /// Watched-folder root this result belongs to.
    pub folder:        String,
    /// True when the query matched a *part* of this image rather than the whole
    /// frame — i.e. this design contains the query design.
    pub partial:       bool,
    /// The part of the image that matched.  Full frame for a whole-image match.
    pub match_region:  MatchRegion,
}

#[derive(Debug, Serialize, Clone)]
pub struct FailedFile {
    pub file:   String,
    pub reason: String,
}

// ── Public API ────────────────────────────────────────────────────────────

/// Execute a visual similarity search across the given folder scope.
///
/// If `scope` is empty, every watched folder in the Library is searched.
/// All folders must already be indexed (the watcher handles this).
pub fn execute(
    image_path: &Path,
    scope:      &[PathBuf],
    top_k:      usize,
) -> Result<(Vec<SearchResult>, Vec<FailedFile>)> {
    // ── Fatal pre-checks ──────────────────────────────────────────────
    if !embedder::is_ready() {
        return Err(PictoriaError::ModelNotReady);
    }
    if !image_path.exists() {
        return Err(PictoriaError::Fatal(
            "The selected reference image no longer exists.".into(),
        ));
    }

    // ── Resolve scope ─────────────────────────────────────────────────
    let con = database::open()?;
    let scope_paths: Vec<String> = if scope.is_empty() {
        database::watched_folder_paths(&con)?
    } else {
        scope.iter().map(|p| p.to_string_lossy().to_string()).collect()
    };
    drop(con);

    if scope_paths.is_empty() {
        return Err(PictoriaError::Fatal(
            "No folders to search.  Add a folder to your Library first.".into(),
        ));
    }

    // ── Embed query image ─────────────────────────────────────────────
    // Done BEFORE acquiring the store lock so ONNX inference (which has its
    // own embedder mutex) never holds the read guard.
    // NOTE: search deliberately does not touch the global progress state.
    // That state is owned by the sync/index pipeline and drives the Library
    // page's progress bar; writing "Searching" here (or calling reset()) would
    // corrupt a concurrent folder index — and reading it back in the search
    // command produced the phantom "Indexing…" bar during search.  The search
    // command emits its own lightweight "Searching" snapshot instead.
    // Loaded at the same resolution the index uses so the query goes through an
    // identical resize chain to the stored vectors it is compared against.
    let query_img   = image_loader::load_image_detailed(image_path)
        .map_err(|e| PictoriaError::Fatal(format!("Could not load reference image: {e}")))?;
    // A scan or product shot often carries a uniform margin.  Left in, that
    // margin is part of the query vector — the search partly looks for "white
    // background" instead of for the design.
    let query_img   = trim_uniform_border(query_img);
    // Colour histogram from full-colour image; design embedding from grayscale so
    // the query vector matches the grayscale-encoded stored vectors (v3 schema).
    let query_color = crate::core::color::histogram(&query_img);

    // The whole frame, plus — for an elongated reference — square windows along
    // its long axis, so the ends a centre crop would drop are still searchable.
    let (qw, qh)  = (query_img.width(), query_img.height());
    let query_plan = crate::core::regions::plan_query(qw, qh);

    let mut pixels: Vec<f32> = Vec::new();
    for r in &query_plan {
        let crop = if r.is_whole() {
            query_img.clone()
        } else {
            let (x, y, cw, ch) = r.to_pixels(qw, qh);
            query_img.crop_imm(x, y, cw, ch)
        };
        pixels.extend(embedder::preprocess_grayscale(crop));
    }

    let query_embs = embedder::embed_batch(&pixels, query_plan.len().max(1))
        .map_err(|e| PictoriaError::Fatal(format!("Failed to embed reference image: {e}")))?;
    if query_embs.is_empty() {
        return Err(PictoriaError::Fatal("Embedding returned empty result".into()));
    }

    // ── Load vector store ─────────────────────────────────────────────
    // Shared read lock — multiple searches can run in parallel and indexing
    // is never blocked.  Only waits if a save is in progress (~2 seconds).
    let _store_guard = crate::core::vector_store::store_io_read_guard();
    let store = VectorStore::load(VECTOR_STORE_PATH.as_path())?;

    // ── Build combined id_map across every folder in scope ────────────
    let con = database::open()?;
    let mut id_map: HashMap<i64, (String, String)> = HashMap::new();
    for folder_str in &scope_paths {
        let folder_id_map = database::folder_id_map(&con, folder_str)?;
        for (id, path) in folder_id_map {
            id_map.insert(id, (path, folder_str.clone()));
        }
    }
    drop(con);

    // ── Vector search.  The store already collapses each file to its single
    //    best-matching region, so this asks for files, not vectors.  Over-fetch
    //    so the post-scope and existence filters below still have enough rows to
    //    fill top_k when the store contains unwatched legacy vectors.
    let raw_k  = top_k.saturating_mul(4).max(top_k);
    let scores = store.search(&query_embs, &query_color, raw_k);

    // Guard against a store that somehow holds the same id/path more than once —
    // never show the same file twice in one result set.
    let mut seen_paths: std::collections::HashSet<String> = std::collections::HashSet::new();

    let scale = |v: f32| (v.clamp(0.0, 1.0) * 100.0 * 10.0).round() / 10.0;

    let results: Vec<SearchResult> = scores
        .into_iter()
        .filter_map(|m| {
            let (path, folder) = id_map.get(&m.id)?.clone();
            if !Path::new(&path).exists() {
                return None;
            }
            if !seen_paths.insert(path.clone()) {
                return None;
            }
            let name = Path::new(&path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&path)
                .to_string();
            let blended = crate::core::vector_store::DESIGN_WEIGHT * m.design_sim
                        + crate::core::vector_store::COLOR_WEIGHT  * m.color_sim;
            let partial = is_partial_match(m.region.is_whole(), m.design_sim, m.whole_sim);
            Some(SearchResult {
                rank:          0,
                path,
                name,
                similarity:    scale(blended),
                pattern_match: scale(m.design_sim),
                color_match:   scale(m.color_sim),
                folder,
                partial,
                match_region:  if partial {
                    MatchRegion { x: m.region.x, y: m.region.y, w: m.region.w, h: m.region.h }
                } else {
                    MatchRegion { x: 0.0, y: 0.0, w: 1.0, h: 1.0 }
                },
            })
        })
        .take(top_k)
        .enumerate()
        .map(|(i, mut r)| { r.rank = i + 1; r })
        .collect();

    Ok((results, Vec::new()))
}

// ── "Found inside" policy ─────────────────────────────────────────────────

/// A region match must clear this before the result is called a partial match.
const PARTIAL_MIN_SCORE: f32 = 0.65;

/// …and it must beat the file's own whole-frame score by at least this much.
///
/// This is the condition that matters.  A file's score is the maximum over up to
/// 16 regions, and a maximum over many samples drifts up by itself — measured on
/// a real library, most results had some region beating their whole frame by
/// 0.16 to 1.4 points, which is noise, and every single result was being
/// labelled "found inside" as a result.  Real containment is unmistakable in the
/// same data: the whole frame matches poorly while one region matches well, a
/// gap of 4 to 9 points (e.g. 86.25 whole vs 93.62 region for an image that
/// genuinely contained the query).  Three points sits well clear of the noise
/// and well below the real cases.
const PARTIAL_MIN_MARGIN: f32 = 0.03;

/// Should this result be presented as "the query was found inside it"?
fn is_partial_match(region_is_whole: bool, best_sim: f32, whole_sim: f32) -> bool {
    if region_is_whole {
        return false; // the whole frame won outright
    }
    if best_sim < PARTIAL_MIN_SCORE {
        return false;
    }
    // A file with no whole-frame entry at all can't be compared; treat the
    // region as the whole story rather than suppressing it.
    if !whole_sim.is_finite() {
        return true;
    }
    best_sim - whole_sim >= PARTIAL_MIN_MARGIN
}

// ── Query preparation ─────────────────────────────────────────────────────

/// Largest fraction of each side we will trim away.
const MAX_TRIM: f32 = 0.25;
/// Per-channel tolerance for "the same colour as the border".
const TRIM_TOL: i32 = 12;

/// Crop a uniform margin (a scanner bed, a white product-shot background) off
/// the reference image.
///
/// Returns the image untouched when there is no clear margin, when the crop
/// would be implausibly aggressive, or when *every* side hits the cap — that
/// last case means the image is uniform throughout (a genuinely plain design),
/// where trimming would just return a smaller piece of the same flat colour.
fn trim_uniform_border(img: DynamicImage) -> DynamicImage {
    let rgb = img.to_rgb8();
    let (w, h) = (rgb.width(), rgb.height());
    if w < 8 || h < 8 {
        return img;
    }

    // Reference colour: the mean of the four corners.  A margin is by
    // definition what the corners are made of.
    let corners = [
        rgb.get_pixel(0, 0),
        rgb.get_pixel(w - 1, 0),
        rgb.get_pixel(0, h - 1),
        rgb.get_pixel(w - 1, h - 1),
    ];
    let mut refc = [0i32; 3];
    for c in 0..3 {
        refc[c] = corners.iter().map(|p| p[c] as i32).sum::<i32>() / 4;
    }

    let matches_ref = |p: &image::Rgb<u8>| -> bool {
        (0..3).all(|c| (p[c] as i32 - refc[c]).abs() <= TRIM_TOL)
    };

    // Sample rather than test every pixel — a margin is uniform by definition,
    // so a coarse scan finds its edge just as reliably and much faster.
    let step_x = (w / 64).max(1);
    let step_y = (h / 64).max(1);

    let row_is_margin = |y: u32| (0..w).step_by(step_x as usize).all(|x| matches_ref(rgb.get_pixel(x, y)));
    let col_is_margin = |x: u32| (0..h).step_by(step_y as usize).all(|y| matches_ref(rgb.get_pixel(x, y)));

    let max_y = (h as f32 * MAX_TRIM) as u32;
    let max_x = (w as f32 * MAX_TRIM) as u32;

    let mut top = 0;
    while top < max_y && row_is_margin(top) { top += 1; }
    let mut bottom = 0;
    while bottom < max_y && row_is_margin(h - 1 - bottom) { bottom += 1; }
    let mut left = 0;
    while left < max_x && col_is_margin(left) { left += 1; }
    let mut right = 0;
    while right < max_x && col_is_margin(w - 1 - right) { right += 1; }

    if top == 0 && bottom == 0 && left == 0 && right == 0 {
        return img;
    }
    // Uniform all the way in on every side → a plain image, not a margin.
    if top >= max_y && bottom >= max_y && left >= max_x && right >= max_x {
        return img;
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

    /// Numbers lifted from a real run against the user's library, where every
    /// single result was being badged "found inside".
    #[test]
    fn noise_sized_region_wins_are_not_partial_matches() {
        // (whole, region) pairs that were wrongly badged.
        for (whole, region, name) in [
            (1.0000, 1.0000, "the query image itself"),
            (0.9326, 0.9351, "ETOILE CREMA-R1.jpg  (+0.25)"),
            (0.9023, 0.9039, "BRASIL GREY P4.jpg   (+0.16)"),
            (0.9019, 0.9073, "MOCHA LIGHT GREY-R1  (+0.54)"),
            (0.9483, 0.9619, "Screenshot 2.09.52   (+1.36)"),
        ] {
            assert!(
                !is_partial_match(false, region, whole),
                "{name} should not be badged: region beat whole by only {:.2} points",
                (region - whole) * 100.0
            );
        }
    }

    #[test]
    fn genuine_containment_is_a_partial_match() {
        for (whole, region, name) in [
            (0.8625, 0.9362, "big image.png       (+7.37)"),
            (0.8149, 0.9050, "FENDER BEIGE_R1.jpg (+9.01)"),
            (0.8820, 0.9257, "ARKOSE-R3.psb       (+4.37)"),
            (0.8587, 0.9013, "ALASKA GREY_F1.jpg  (+4.26)"),
        ] {
            assert!(
                is_partial_match(false, region, whole),
                "{name} should be badged: whole frame matches poorly, one region matches well"
            );
        }
    }

    #[test]
    fn a_whole_frame_win_is_never_partial() {
        assert!(!is_partial_match(true, 0.99, 0.99));
    }

    #[test]
    fn weak_matches_are_never_partial() {
        // Clears the margin easily but is too weak to claim anything.
        assert!(!is_partial_match(false, 0.40, 0.10));
    }
}
