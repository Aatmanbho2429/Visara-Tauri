//! Region planning for partial-design search.
//!
//! ## The problem
//! A single embedding summarises a whole image, so a small motif that occupies
//! a ninth of a large carpet contributes roughly a ninth of the signal.  Query
//! with the motif on its own and the carpet scores low — not because the model
//! is wrong, but because "are these the same picture?" is a different question
//! from "does this picture contain that one?".
//!
//! ## The fix
//! Index each image as several vectors instead of one: the whole frame, plus a
//! grid of overlapping windows.  A window that happens to frame the motif gets
//! its own embedding, which *does* match the query.  Search then scores a file
//! by its best-matching region (see [`crate::core::vector_store::search`]).
//!
//! ## Why more than one window size
//! Every window is resized to 224 before the model sees it, so what matters is
//! how the design is *framed*, not the pixel count.  The motif might be a
//! quarter of the carpet or a twenty-fifth, and we can't know which, so windows
//! are emitted at several scales and one of them ends up framing the motif
//! roughly the way the standalone tile does.
//!
//! Windows overlap so a motif sitting on a grid line still lands whole inside
//! at least one of them.

use crate::config::CLIP_INPUT_SIZE;

/// A window of an image, in normalised coordinates (0.0–1.0 of width/height).
///
/// Normalised rather than pixels so the value survives the image being loaded
/// at a different resolution later, and so the UI can draw the match box over a
/// thumbnail of any size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Region {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Region {
    /// The entire image — what every file gets in addition to its slices, and
    /// what a query image is embedded as.
    pub const WHOLE: Region = Region { x: 0.0, y: 0.0, w: 1.0, h: 1.0 };

    /// True when this region covers (essentially) the whole frame.  Used to tell
    /// a full-image match from a partial one in search results.
    pub fn is_whole(&self) -> bool {
        self.w >= 0.999 && self.h >= 0.999
    }

    /// Convert to pixel coordinates within an image of the given size, clamped
    /// so the window always lies inside the frame.
    pub fn to_pixels(&self, img_w: u32, img_h: u32) -> (u32, u32, u32, u32) {
        let x = (self.x * img_w as f32).round().max(0.0) as u32;
        let y = (self.y * img_h as f32).round().max(0.0) as u32;
        let w = (self.w * img_w as f32).round().max(1.0) as u32;
        let h = (self.h * img_h as f32).round().max(1.0) as u32;
        let x = x.min(img_w.saturating_sub(1));
        let y = y.min(img_h.saturating_sub(1));
        (x, y, w.min(img_w - x).max(1), h.min(img_h - y).max(1))
    }
}

/// Window sizes to emit, as a fraction of the image's *shorter* side.
///
/// `1.0` produces squares of the short side, which on an elongated image walk
/// along the long axis — that is what gives full coverage of a plank or runner
/// (on a square image it is the whole frame, and gets dropped as a duplicate).
/// `0.5` is the level that catches a motif making up somewhere between a
/// quarter and a ninth of a composite, which is the common case for tile
/// carpets.
///
/// Adding a third, smaller scale here is the one-line change that extends this
/// to motifs smaller than about a ninth of the frame.  It is left out by
/// default because each extra level multiplies the stored vectors per image
/// (a third level roughly triples them).
const SCALES: [f32; 2] = [1.0, 0.5];

/// Fraction of a window's size to step by.  0.5 = 50% overlap, so any motif up
/// to one window wide lands entirely inside some window regardless of where the
/// grid lines fall.
const STEP_RATIO: f32 = 0.5;

/// Smallest window worth cutting, in source pixels.
///
/// Below the model's input size the window would have to be *upscaled* to 224,
/// which invents no detail and just produces a blurry near-duplicate of a
/// coarser window.  This is also what makes the planner self-limiting on
/// low-resolution sources: a 512px image simply gets fewer levels.
const MIN_REGION_PX: u32 = CLIP_INPUT_SIZE;

/// Hard ceiling on regions per image, as a safety valve on index size.
/// Coarser scales are emitted first, so truncation drops the finest windows.
const MAX_REGIONS: usize = 16;

/// Plan the set of regions to embed for an image of the given pixel size.
///
/// Always returns at least [`Region::WHOLE`], so this is safe to use for every
/// image — a small single-motif tile just gets the one region it had before.
pub fn plan(img_w: u32, img_h: u32) -> Vec<Region> {
    let mut out = vec![Region::WHOLE];
    if img_w == 0 || img_h == 0 {
        return out;
    }

    let short = img_w.min(img_h);

    for &scale in SCALES.iter() {
        let side = ((short as f32) * scale).round() as u32;
        if side < MIN_REGION_PX {
            continue; // would upscale — no detail to gain
        }
        let step = (((side as f32) * STEP_RATIO).round() as u32).max(1);

        for y in axis_positions(img_h, side, step) {
            for x in axis_positions(img_w, side, step) {
                let r = Region {
                    x: x as f32 / img_w as f32,
                    y: y as f32 / img_h as f32,
                    w: side as f32 / img_w as f32,
                    h: side as f32 / img_h as f32,
                };
                // A window equal to the frame is already covered by WHOLE.
                if r.is_whole() {
                    continue;
                }
                out.push(r);
                if out.len() >= MAX_REGIONS {
                    return out;
                }
            }
        }
    }

    out
}

/// Aspect ratio beyond which a query image is also cut into windows.
///
/// Below this a centre crop already covers essentially the whole design, so the
/// extra vectors would only cost search time.
const QUERY_ASPECT_MIN: f32 = 1.2;

/// Plan the windows to embed for a **query** image.
///
/// Deliberately much shallower than [`plan`]: the whole frame, plus — only for
/// an elongated query — square windows walking along its long axis.
///
/// The reason is the centre crop in [`crate::core::embedder::preprocess`].  For
/// a 1:2 reference tile that crop keeps the middle square and drops the ends,
/// so a design feature near either end would never reach the model.  Walking
/// square windows along the long axis restores those ends.
///
/// The fine scales that [`plan`] uses for indexing are deliberately *not*
/// applied here.  Matching on a small fragment of the query would ask "find
/// anything containing any piece of this tile", which is a far looser question
/// than the one the user asked and pulls in a lot of noise.
pub fn plan_query(img_w: u32, img_h: u32) -> Vec<Region> {
    let mut out = vec![Region::WHOLE];
    if img_w == 0 || img_h == 0 {
        return out;
    }

    let short = img_w.min(img_h);
    let long  = img_w.max(img_h);
    if (long as f32) / (short as f32) < QUERY_ASPECT_MIN {
        return out; // a centre crop already covers it
    }
    if short < MIN_REGION_PX {
        return out;
    }

    let side = short;
    let step = (side / 2).max(1);
    for y in axis_positions(img_h, side, step) {
        for x in axis_positions(img_w, side, step) {
            let r = Region {
                x: x as f32 / img_w as f32,
                y: y as f32 / img_h as f32,
                w: side as f32 / img_w as f32,
                h: side as f32 / img_h as f32,
            };
            if r.is_whole() {
                continue;
            }
            out.push(r);
        }
    }
    out
}

/// Window origins along one axis: stepped by `step`, plus a final window flush
/// against the far edge so the last strip of the image is never left uncovered.
///
/// The flush window is skipped when it would sit almost on top of the previous
/// one, which would otherwise store a near-duplicate vector.
fn axis_positions(extent: u32, side: u32, step: u32) -> Vec<u32> {
    if side >= extent {
        return vec![0];
    }

    let mut pos = Vec::new();
    let mut p = 0u32;
    while p + side <= extent {
        pos.push(p);
        p += step;
    }
    if pos.is_empty() {
        pos.push(0);
    }

    let flush = extent - side;
    let last = *pos.last().unwrap();
    if flush > last && flush - last > side / 8 {
        pos.push(flush);
    }
    pos
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_image_gets_only_the_whole_frame() {
        // Below MIN_REGION_PX at every scale, so there is nothing worth slicing.
        let r = plan(300, 300);
        assert_eq!(r.len(), 1);
        assert!(r[0].is_whole());
    }

    #[test]
    fn square_image_gets_whole_plus_a_grid() {
        let r = plan(2048, 2048);
        assert!(r[0].is_whole());
        // scale 1.0 collapses to the whole frame (dropped); scale 0.5 gives 3x3.
        assert_eq!(r.len(), 10);
        assert!(r[1..].iter().all(|x| !x.is_whole()));
    }

    #[test]
    fn regions_stay_inside_the_frame() {
        for (w, h) in [(2048, 2048), (1024, 3072), (4000, 1500), (900, 640)] {
            for r in plan(w, h) {
                let (px, py, pw, ph) = r.to_pixels(w, h);
                assert!(px + pw <= w, "{w}x{h} region overflows width");
                assert!(py + ph <= h, "{w}x{h} region overflows height");
                assert!(pw > 0 && ph > 0);
            }
        }
    }

    #[test]
    fn elongated_image_is_covered_end_to_end() {
        // A 1:3 runner: scale 1.0 walks square windows along the long axis, and
        // the flush window must reach the far end.
        let r = plan(1000, 3000);
        let max_reach = r
            .iter()
            .map(|x| x.y + x.h)
            .fold(0.0_f32, f32::max);
        assert!(max_reach > 0.99, "far edge never covered: {max_reach}");
    }

    #[test]
    fn never_exceeds_the_cap() {
        assert!(plan(6000, 6000).len() <= MAX_REGIONS);
    }

    #[test]
    fn a_square_query_is_just_the_whole_frame() {
        // A centre crop already covers it, so extra windows would only cost
        // search time.
        let r = plan_query(2000, 2000);
        assert_eq!(r.len(), 1);
        assert!(r[0].is_whole());
    }

    #[test]
    fn an_elongated_query_gets_windows_along_its_long_axis() {
        // The 324x642 reference tile from the real reproduction case: the centre
        // crop keeps only the middle square, so both ends need their own window.
        let r = plan_query(324, 642);
        assert!(r.len() > 1, "elongated query must be windowed, got {}", r.len());
        assert!(r[0].is_whole());

        // Windows walk down the long axis and must reach the far end.
        let reach = r.iter().map(|x| x.y + x.h).fold(0.0_f32, f32::max);
        assert!(reach > 0.99, "far end of the query never covered: {reach}");

        // No fine-grained fragments — that would loosen the query into "find
        // anything containing any piece of this tile".
        for w in r.iter().filter(|x| !x.is_whole()) {
            assert!(w.w > 0.99, "query windows span the short axis fully");
        }
    }

    #[test]
    fn a_tiny_query_is_never_windowed() {
        // Below MIN_REGION_PX the windows would be upscaled, gaining nothing.
        let r = plan_query(80, 200);
        assert_eq!(r.len(), 1);
    }

    /// The reproduction case, as geometry: a carpet laid out as a 3x3 grid of
    /// distinct motifs.  For the search to find it from one motif, some window
    /// must contain that motif *whole* and frame it reasonably tightly — a
    /// window that merely clips it, or one so large the motif is a speck inside
    /// it, will not embed to anything close to the standalone tile.
    #[test]
    fn every_motif_of_a_composite_is_framed_by_some_window() {
        let (cw, ch) = (2048_u32, 2048_u32);
        let plan = plan(cw, ch);
        let cell = 1.0 / 3.0;

        for gy in 0..3 {
            for gx in 0..3 {
                let (mx, my) = (gx as f32 * cell, gy as f32 * cell);
                let (mx2, my2) = (mx + cell, my + cell);

                let framed = plan.iter().any(|r| {
                    if r.is_whole() {
                        return false; // the whole frame is the case that already fails
                    }
                    let contains = r.x <= mx + 1e-3
                        && r.y <= my + 1e-3
                        && r.x + r.w >= mx2 - 1e-3
                        && r.y + r.h >= my2 - 1e-3;
                    // How much of the window the motif occupies.  Too small and
                    // the motif is diluted all over again, just less severely.
                    let fill = (cell * cell) / (r.w * r.h).max(1e-6);
                    contains && fill >= 0.3
                });

                assert!(framed, "motif at grid ({gx},{gy}) is not framed by any window");
            }
        }
    }
}
