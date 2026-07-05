//! Dominant-colour extraction for tiles.
//!
//! Tiles are mostly low-saturation neutrals (whites, creams, greys, browns), so
//! a plain RGB nearest-colour match confuses beige/grey/cream badly.  Instead we
//! classify every (down-sampled) pixel in HSV — saturation first to split
//! neutrals from chromatics — vote, and return the dominant bucket(s).

use image::DynamicImage;

/// Longest edge the image is shrunk to before counting colours.  Small is fine:
/// we only need the overall palette, and this keeps it fast.
const SAMPLE_MAX: u32 = 48;

/// Classify the dominant colour(s) of an image.  Returns 1–2 bucket names
/// (primary first), e.g. `["beige"]` or `["grey", "white"]`.
pub fn dominant(img: &DynamicImage) -> Vec<String> {
    let small = img.resize(SAMPLE_MAX, SAMPLE_MAX, image::imageops::FilterType::Triangle);
    let rgb = small.to_rgb8();

    let mut votes: std::collections::HashMap<&'static str, u32> = std::collections::HashMap::new();
    for p in rgb.pixels() {
        let bucket = classify(p[0], p[1], p[2]);
        *votes.entry(bucket).or_insert(0) += 1;
    }
    if votes.is_empty() {
        return Vec::new();
    }

    let total: u32 = votes.values().sum();
    let mut ranked: Vec<(&'static str, u32)> = votes.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1));

    let mut out = Vec::new();
    out.push(ranked[0].0.to_string());
    // Include a second colour only if it is a meaningful share (>= 25%) and
    // distinct from the first — captures e.g. a grey tile with strong white veins.
    if let Some(&(name, count)) = ranked.get(1) {
        if count * 100 >= total * 25 {
            out.push(name.to_string());
        }
    }
    out
}

// ── Colour similarity vector ────────────────────────────────────────────────

/// Fixed bucket order for the colour histogram vector.  MUST list every string
/// `classify` can return, and MUST NOT be reordered once shipped — the index of
/// each bucket is the dimension it occupies in the stored colour vector, so a
/// reorder would silently invalidate every persisted vector.
pub const COLOR_BUCKETS: [&str; 16] = [
    "white", "light grey", "grey", "charcoal", "black",
    "cream", "beige",
    "red", "brown", "terracotta", "gold",
    "green", "teal", "blue", "purple", "pink",
];

/// Dimensionality of the colour vector produced by [`histogram`].
pub const COLOR_DIM: usize = COLOR_BUCKETS.len();

/// Longest edge used when building the colour histogram.  Slightly larger than
/// `SAMPLE_MAX` so the palette is well sampled without being expensive.
const HIST_MAX: u32 = 64;

/// Build an L2-normalised colour histogram over [`COLOR_BUCKETS`] for one image.
///
/// The image is white-balanced (gray-world) before bucketing so that the same
/// design photographed/rendered under a warm vs cool light lands in the same
/// palette buckets — otherwise "beige under warm light" and "beige under neutral
/// light" would look like different colours to the search.
///
/// Returns a `COLOR_DIM`-length vector; the zero vector when the image is empty
/// (its inner product with any query is 0, i.e. it simply contributes no colour
/// signal rather than corrupting the score).
pub fn histogram(img: &DynamicImage) -> Vec<f32> {
    let small = img.resize(HIST_MAX, HIST_MAX, image::imageops::FilterType::Triangle);
    let rgb = small.to_rgb8();

    // ── Gray-world white balance ──────────────────────────────────────────
    let (mut sum_r, mut sum_g, mut sum_b) = (0f64, 0f64, 0f64);
    let n = rgb.pixels().len().max(1) as f64;
    for p in rgb.pixels() {
        sum_r += p[0] as f64;
        sum_g += p[1] as f64;
        sum_b += p[2] as f64;
    }
    let (mr, mg, mb) = (sum_r / n, sum_g / n, sum_b / n);
    let gray = (mr + mg + mb) / 3.0;
    let scale = |mean: f64| if mean > 1.0 { gray / mean } else { 1.0 };
    let (kr, kg, kb) = (scale(mr), scale(mg), scale(mb));

    let mut hist = vec![0f32; COLOR_DIM];
    for p in rgb.pixels() {
        let r = ((p[0] as f64 * kr).round() as i64).clamp(0, 255) as u8;
        let g = ((p[1] as f64 * kg).round() as i64).clamp(0, 255) as u8;
        let b = ((p[2] as f64 * kb).round() as i64).clamp(0, 255) as u8;
        let bucket = classify(r, g, b);
        if let Some(idx) = COLOR_BUCKETS.iter().position(|&name| name == bucket) {
            hist[idx] += 1.0;
        }
    }

    // L2-normalise so the colour vector's inner product is a cosine in [0, 1],
    // matching how the design (CLIP) vector is compared.
    let norm = hist.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in hist.iter_mut() {
            *v /= norm;
        }
    }
    hist
}

/// Map one RGB sample to a named bucket via HSV rules.
fn classify(r: u8, g: u8, b: u8) -> &'static str {
    let (h, s, v) = rgb_to_hsv(r, g, b);

    // ── Neutrals (low saturation): decide by lightness ───────────────────
    if s < 0.14 {
        return match v {
            x if x >= 0.85 => "white",
            x if x >= 0.62 => "light grey",
            x if x >= 0.32 => "grey",
            x if x >= 0.12 => "charcoal",
            _              => "black",
        };
    }

    // ── Muted, light, warm tones → the beige / cream / tan family ─────────
    if s < 0.34 && v >= 0.5 && (20.0..70.0).contains(&h) {
        return if v >= 0.8 { "cream" } else { "beige" };
    }

    // ── Chromatic: decide by hue ─────────────────────────────────────────
    match h {
        x if !(15.0..345.0).contains(&x) => "red",
        x if (15.0..45.0).contains(&x)   => if v < 0.5 { "brown" } else { "terracotta" },
        x if (45.0..70.0).contains(&x)   => if v < 0.5 { "brown" } else { "gold" },
        x if (70.0..170.0).contains(&x)  => "green",
        x if (170.0..200.0).contains(&x) => "teal",
        x if (200.0..260.0).contains(&x) => "blue",
        x if (260.0..300.0).contains(&x) => "purple",
        _                                => "pink",
    }
}

/// Returns (hue 0–360, saturation 0–1, value 0–1).
fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let rf = r as f32 / 255.0;
    let gf = g as f32 / 255.0;
    let bf = b as f32 / 255.0;

    let max = rf.max(gf).max(bf);
    let min = rf.min(gf).min(bf);
    let delta = max - min;

    let v = max;
    let s = if max <= 0.0 { 0.0 } else { delta / max };

    let h = if delta <= 0.0 {
        0.0
    } else if max == rf {
        60.0 * (((gf - bf) / delta) % 6.0)
    } else if max == gf {
        60.0 * (((bf - rf) / delta) + 2.0)
    } else {
        60.0 * (((rf - gf) / delta) + 4.0)
    };
    let h = if h < 0.0 { h + 360.0 } else { h };

    (h, s, v)
}
