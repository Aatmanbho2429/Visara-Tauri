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
