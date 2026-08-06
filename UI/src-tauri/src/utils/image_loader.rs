//! Smart image loading that handles every format Pictoria indexes:
//! JPEG, PNG, TIFF (including multi-page), PSD, and PSB.
//!
//! All functions return a decoded `image::DynamicImage` in RGB8 colour space,
//! ready for the CLIP pre-processing pipeline.

use crate::error::{Result, PictoriaError};
use image::{DynamicImage, ImageReader};
use std::{
    io::{BufReader, Cursor, Read, Seek, SeekFrom},
    path::Path,
};

// ── Public entry point ────────────────────────────────────────────────────

/// Load any supported image format from disk and return it as `DynamicImage`.
///
/// Strategy per format:
/// - **JPEG / PNG / BMP / WebP / GIF** — decoded directly by the `image` crate.
/// - **TIFF** — tries the embedded IFD1 thumbnail first; falls back to the
///   main image with an integer downscale hint to avoid loading huge raw files.
/// - **PSD / PSB** — tries the embedded JPEG thumbnail in the resource section
///   (fast, avoids full composite render); falls back to `image` crate decode.
pub fn load_image(path: &Path) -> Result<DynamicImage> {
    load_image_at(path, TIFF_TARGET_MAX)
}

/// Load at a resolution high enough to be sliced into regions.
///
/// The whole-image embedding only ever needs 224px, so [`load_image`] happily
/// returns a 512px reduction.  Region slicing is different: a 1/9 window of a
/// 512px carpet is ~170px, which then has to be *upscaled* to 224 — blurrier
/// than the original and useless for matching.  Cutting the same window out of
/// a 2048px load gives ~680px of real detail to downsample from instead.
///
/// This costs more than [`load_image`], but far less than the ratio suggests:
/// the expensive part for a large TIFF/PSB is decompressing the source, which
/// is paid either way.  Only the resample and (for `sips`) the intermediate
/// PNG grow.
pub fn load_image_detailed(path: &Path) -> Result<DynamicImage> {
    load_image_at(path, INDEX_DETAIL_MAX)
}

fn load_image_at(path: &Path, target_max: u32) -> Result<DynamicImage> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let primary = match ext.as_str() {
        "psd" | "psb" => load_psd_psb(path, target_max),
        "tif" | "tiff" => load_tiff(path, target_max),
        _ => load_standard(path),
    };

    match primary {
        Ok(img) => Ok(img),
        Err(e) => {
            // Last resort on macOS: hand the file to the OS image tooling, which
            // reads variants our in-process decoders reject (6-channel CMYK
            // TIFFs, large PSBs, HEIC…) and resamples to a small size cheaply —
            // i.e. "reduce, then embed".
            #[cfg(target_os = "macos")]
            if let Ok(img) = load_via_sips(path, target_max) {
                log::info!("[image] decoded via sips fallback: {:?}", path);
                return Ok(img);
            }
            Err(e)
        }
    }
}

/// macOS fallback: render any image the OS can read into a small downscaled PNG
/// we can embed.  Spawns `sips`, which streams the conversion (low memory) and
/// applies proper colour management for CMYK.
#[cfg(target_os = "macos")]
fn load_via_sips(path: &Path, target_max: u32) -> Result<DynamicImage> {
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicU64, Ordering};

    // Unique temp path per call so parallel sync workers don't clobber each other.
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let tmp = std::env::temp_dir().join(format!(
        "pictoria_sips_{}_{}.png",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed),
    ));

    let status = Command::new("/usr/bin/sips")
        .args(["-s", "format", "png"])
        .arg(path)
        .arg("--out")
        .arg(&tmp)
        .args(["--resampleHeightWidthMax", &target_max.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| PictoriaError::Fatal(format!("sips spawn failed: {e}")))?;

    if !status.success() {
        let _ = std::fs::remove_file(&tmp);
        return Err(PictoriaError::Fatal("sips could not convert image".into()));
    }

    let result = load_standard(&tmp);
    let _ = std::fs::remove_file(&tmp);
    result
}

// ── Standard formats ──────────────────────────────────────────────────────

fn load_standard(path: &Path) -> Result<DynamicImage> {
    Ok(ImageReader::open(path)?.with_guessed_format()?.decode()?)
}

// ── TIFF ──────────────────────────────────────────────────────────────────

fn load_tiff(path: &Path, target_max: u32) -> Result<DynamicImage> {
    // CMYK TIFFs are device-dependent and need ICC colour management to convert
    // accurately.  Our in-Rust decode applies only a naive CMYK→RGB formula,
    // which casts neutral marbles green/pink/yellow.  On macOS, sips is
    // colour-managed (ColorSync) and cheap, so prefer it for TIFFs.
    #[cfg(target_os = "macos")]
    if let Ok(img) = load_via_sips(path, target_max) {
        return Ok(img);
    }

    // Non-macOS path: try sources in order of colour accuracy.
    //
    // 1. Photoshop JPEG preview (TIFF tag 34377) — Photoshop renders this with
    //    ICC-managed colour conversion, so CMYK → sRGB is perceptually correct
    //    without needing an ICC library on our side.
    //
    //    Skipped when we need detail: these previews are small (typically a few
    //    hundred pixels), so accepting one here would silently cap the
    //    resolution available for region slicing.  We only fall back to it if
    //    the real decode fails.
    if target_max <= TIFF_TARGET_MAX {
        if let Ok(img) = try_tiff_photoshop_preview(path) {
            return Ok(img);
        }
        // 2. IFD1 thumbnail — usually RGB, small, accurate enough.
        if let Ok(thumb) = try_tiff_thumbnail(path) {
            return Ok(thumb);
        }
    }

    // 3. Full pixel decode with our naive CMYK → RGB formula (colour may drift
    //    vs. the OS for CMYK files, but this is the last resort).
    match load_tiff_main(path, target_max) {
        Ok(img) => Ok(img),
        Err(e) if target_max > TIFF_TARGET_MAX => {
            // Detail decode failed — a low-resolution preview still beats
            // dropping the file entirely.  It just won't be worth slicing.
            log::debug!("[tiff] detail decode failed ({e}); falling back to preview");
            try_tiff_photoshop_preview(path)
                .or_else(|_| try_tiff_thumbnail(path))
                .or(Err(e))
        }
        Err(e) => Err(e),
    }
}

fn try_tiff_thumbnail(path: &Path) -> Result<DynamicImage> {
    use tiff::decoder::Decoder;

    let file = std::fs::File::open(path)?;
    let mut decoder = Decoder::new(file).map_err(|e| {
        PictoriaError::Fatal(format!("TIFF decoder init failed: {e}"))
    })?;

    // IFD0 is the main image; IFD1 is the thumbnail when present.
    if !decoder.more_images() {
        return Err(PictoriaError::Fatal("No IFD1 thumbnail".into()));
    }
    decoder.next_image().map_err(|e| PictoriaError::Fatal(e.to_string()))?;

    let (w, h) = decoder.dimensions().map_err(|e| PictoriaError::Fatal(e.to_string()))?;
    if w.max(h) > 2048 {
        return Err(PictoriaError::Fatal("IFD1 too large".into()));
    }

    let result = decoder.read_image().map_err(|e| PictoriaError::Fatal(e.to_string()))?;
    tiff_result_to_dynamic(result, w as u32, h as u32)
}

/// Extract the Photoshop-embedded JPEG preview from a TIFF file.
///
/// Photoshop stores a JPEG thumbnail in TIFF tag 34377 (the "Photoshop" private
/// tag) using exactly the same 8BIM image-resources format as PSD/PSB files.
/// Crucially, Photoshop renders this thumbnail *with ICC colour management*, so
/// the CMYK → sRGB conversion is perceptually correct — no colour drift.
fn try_tiff_photoshop_preview(path: &Path) -> Result<DynamicImage> {
    let f = std::fs::File::open(path)?;
    let mut r = BufReader::new(f);
    let ps_data = tiff_read_tag_34377(&mut r)?;
    parse_8bim_jpeg_preview(&ps_data)
}

/// Walk the TIFF IFD0 looking for tag 34377 and return its raw bytes.
fn tiff_read_tag_34377<R: Read + Seek>(r: &mut R) -> Result<Vec<u8>> {
    r.seek(SeekFrom::Start(0))?;

    let mut bo = [0u8; 2];
    r.read_exact(&mut bo)?;
    let le = match &bo {
        b"II" => true,
        b"MM" => false,
        _ => return Err(PictoriaError::Fatal("Not a TIFF".into())),
    };

    if tiff_ru16(r, le)? != 42 {
        return Err(PictoriaError::Fatal("TIFF magic mismatch".into()));
    }

    let ifd0_off = tiff_ru32(r, le)?;
    r.seek(SeekFrom::Start(ifd0_off as u64))?;
    let n = tiff_ru16(r, le)?;

    for _ in 0..n {
        let tag   = tiff_ru16(r, le)?;
        let _ty   = tiff_ru16(r, le)?;
        let count = tiff_ru32(r, le)?;
        let mut val = [0u8; 4];
        r.read_exact(&mut val)?;

        if tag == 34377 && count > 4 {
            let offset = if le { u32::from_le_bytes(val) } else { u32::from_be_bytes(val) };
            r.seek(SeekFrom::Start(offset as u64))?;
            let mut data = vec![0u8; count as usize];
            r.read_exact(&mut data)?;
            return Ok(data);
        }
    }

    Err(PictoriaError::Fatal("Tag 34377 not found in TIFF IFD0".into()))
}

fn tiff_ru16<R: Read>(r: &mut R, le: bool) -> Result<u16> {
    let mut b = [0u8; 2];
    r.read_exact(&mut b)?;
    Ok(if le { u16::from_le_bytes(b) } else { u16::from_be_bytes(b) })
}

fn tiff_ru32<R: Read>(r: &mut R, le: bool) -> Result<u32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(if le { u32::from_le_bytes(b) } else { u32::from_be_bytes(b) })
}

/// Parse a sequence of Photoshop 8BIM resource blocks and return the first
/// JPEG thumbnail found (resource IDs 1033 or 1036).  Used for both the TIFF
/// tag-34377 path and the PSD/PSB image-resources section.
fn parse_8bim_jpeg_preview(data: &[u8]) -> Result<DynamicImage> {
    let mut cur = Cursor::new(data);

    while cur.position() + 12 <= data.len() as u64 {
        let mut bim = [0u8; 4];
        if cur.read_exact(&mut bim).is_err() { break; }
        if &bim != b"8BIM" { break; }

        let res_id   = read_u16_be(&mut cur)?;
        let name_len = read_u8(&mut cur)? as u64;
        let pad      = if (name_len + 1) % 2 != 0 { 1u64 } else { 0u64 };
        cur.seek(SeekFrom::Current((name_len + pad) as i64))?;

        let data_len = read_u32_be(&mut cur)? as u64;
        let data_end = cur.position() + data_len + (data_len % 2);

        if (res_id == 1033 || res_id == 1036) && data_len > 28 {
            cur.seek(SeekFrom::Current(28))?;
            let jpeg_len = (data_len - 28) as usize;
            let mut jpeg_bytes = vec![0u8; jpeg_len];
            cur.read_exact(&mut jpeg_bytes)?;
            let img = image::load_from_memory_with_format(
                &jpeg_bytes,
                image::ImageFormat::Jpeg,
            )?;
            return Ok(img);
        }

        if data_end > data.len() as u64 { break; }
        cur.seek(SeekFrom::Start(data_end))?;
    }

    Err(PictoriaError::Fatal("No JPEG thumbnail in Photoshop 8BIM blocks".into()))
}

/// Down-scaled longest edge fed toward the CLIP pipeline (which finally wants
/// 224px).  Sampling to this size straight out of the decoded TIFF buffer avoids
/// allocating a second full-resolution image.
const TIFF_TARGET_MAX: u32 = 512;

/// Longest edge used when an image is being indexed for region slicing.
///
/// Sized so a 1/5-scale window (the smallest level the region planner emits)
/// still yields ~400px of source for the 224px model input, i.e. every slice
/// is downsampled rather than upscaled.
pub const INDEX_DETAIL_MAX: u32 = 2048;

fn load_tiff_main(path: &Path, target_max: u32) -> Result<DynamicImage> {
    // Primary: decode with the `tiff` crate directly so we can lift its 256 MB
    // buffer cap (large multi-channel design TIFFs — e.g. 8008x15709 CMYK ≈
    // 480 MB — were otherwise rejected with "Memory limit exceeded") and convert
    // CMYK ourselves.  Fall back to the image crate for compressions the `tiff`
    // crate can't handle (e.g. some JPEG-in-TIFF variants).
    match load_tiff_via_tiff_crate(path, target_max) {
        Ok(img) => Ok(img),
        Err(e)  => {
            log::debug!("[tiff] direct decode failed ({e}); falling back to image crate");
            let img = ImageReader::open(path)?.with_guessed_format()?.decode()?;
            Ok(downscale(img, target_max))
        }
    }
}

fn load_tiff_via_tiff_crate(path: &Path, target_max: u32) -> Result<DynamicImage> {
    use image::{ImageBuffer, Rgb};
    use tiff::decoder::{DecodingResult, Decoder, Limits};
    use tiff::ColorType;

    let file = std::fs::File::open(path)?;
    let mut decoder = Decoder::new(file)
        .map_err(|e| PictoriaError::Fatal(format!("TIFF open failed: {e}")))?
        .with_limits(Limits::unlimited());

    let (w, h) = decoder
        .dimensions()
        .map_err(|e| PictoriaError::Fatal(format!("TIFF dimensions failed: {e}")))?;

    // colortype() fails for multi-channel CMYK variants (e.g. CMYK + spot-color
    // channels from Photoshop/Illustrator).  Treat unknown as CMYK(8); sample_to_rgb
    // takes only the first 4 channels (C,M,Y,K) and ignores any extras.
    let color = decoder.colortype().unwrap_or(ColorType::CMYK(8));

    // ── Fast path: partial strip reading for large strip-based TIFFs ──────
    // Decompressing an 8 000 × 16 000 CMYK TIFF at full resolution takes several
    // seconds.  For a pattern/texture embedding we only need TIFF_TARGET_MAX rows —
    // tile designs repeat, so the first 512 rows carry the full design signal.
    // strip_count() fails (returns Err) for tile-based TIFFs; that falls through
    // to the full read_image() path below, which is fine because tiles are usually
    // smaller files.
    //
    // NOT valid when indexing for region slicing.  "The design repeats" holds for
    // a single tile but is exactly false for a composite — a carpet built from a
    // grid of *different* motifs.  Reading only the top strips would leave the
    // lower two-thirds of such a file permanently unindexed, which surfaces as a
    // search that mysteriously fails to find the right image rather than as a
    // load error.  In detail mode we always read the full height.
    if target_max <= TIFF_TARGET_MAX && w.max(h) > TIFF_TARGET_MAX * 2 {
        if let Ok(n_strips) = decoder.strip_count() {
            if n_strips > 1 {
                let (_, rows_per_strip) = decoder.chunk_dimensions();
                let strips_needed = ((target_max / rows_per_strip.max(1)) + 1)
                    .min(n_strips);

                let mut raw: Vec<u8> = Vec::new();
                let mut partial_ok = true;

                for i in 0..strips_needed {
                    match decoder.read_chunk(i) {
                        Ok(DecodingResult::U8(d))  => raw.extend(d),
                        Ok(DecodingResult::U16(d)) => {
                            raw.extend(d.iter().map(|&v| (v >> 8) as u8));
                        }
                        _ => { partial_ok = false; break; }
                    }
                }

                if partial_ok && !raw.is_empty() {
                    let avail_h  = (rows_per_strip * strips_needed).min(h);
                    let total_px = w as usize * avail_h as usize;
                    let channels = (raw.len() / total_px.max(1)).max(1);

                    let mut rgb = Vec::with_capacity(total_px * 3);
                    for py in 0..avail_h as usize {
                        for px in 0..w as usize {
                            let base = (py * w as usize + px) * channels;
                            let (r, g, b) = if base + channels <= raw.len() {
                                sample_to_rgb(&raw[base..base + channels], channels, color)
                            } else {
                                (0, 0, 0)
                            };
                            rgb.extend_from_slice(&[r, g, b]);
                        }
                    }

                    if let Some(buf) =
                        ImageBuffer::<Rgb<u8>, _>::from_raw(w, avail_h, rgb)
                    {
                        // Return the partial-height RGB image.  Downstream
                        // preprocess_grayscale / color::histogram both resize to
                        // their own targets, so the reduced height is fine.
                        return Ok(DynamicImage::ImageRgb8(buf));
                    }
                }
            }
        }
    }

    // ── Full decode: small TIFFs, tile-based, or partial-strip fallback ───
    let result = decoder
        .read_image()
        .map_err(|e| PictoriaError::Fatal(format!("TIFF read failed: {e}")))?;

    tiff_to_downscaled_rgb(result, w, h, color, target_max)
}

/// Convert a decoded TIFF buffer to a down-sampled RGB image in a single pass.
/// Sampling with an integer stride out of the source buffer keeps peak memory at
/// just the decoded buffer (no intermediate full-resolution RGB copy).
fn tiff_to_downscaled_rgb(
    result:     tiff::decoder::DecodingResult,
    w:          u32,
    h:          u32,
    color:      tiff::ColorType,
    target_max: u32,
) -> Result<DynamicImage> {
    use image::{ImageBuffer, Rgb};
    use tiff::decoder::DecodingResult;

    if w == 0 || h == 0 {
        return Err(PictoriaError::Fatal("TIFF has zero dimensions".into()));
    }

    // Normalise samples to 8-bit (16-bit scaled down by a byte).
    let data: Vec<u8> = match result {
        DecodingResult::U8(d)  => d,
        DecodingResult::U16(d) => d.iter().map(|&v| (v >> 8) as u8).collect(),
        _ => return Err(PictoriaError::Fatal("Unsupported TIFF sample format".into())),
    };

    let px = (w as usize) * (h as usize);
    let channels = data.len() / px;
    if channels == 0 {
        return Err(PictoriaError::Fatal("TIFF sample/byte count mismatch".into()));
    }

    let stride     = (w.max(h) / target_max).max(1) as usize;
    let ow         = ((w as usize) / stride).max(1);
    let oh         = ((h as usize) / stride).max(1);
    let row_stride = (w as usize) * channels;

    let mut out = vec![0u8; ow * oh * 3];
    for oy in 0..oh {
        let sy = oy * stride;
        for ox in 0..ow {
            let base = sy * row_stride + (ox * stride) * channels;
            if base + channels > data.len() { continue; }
            let (r, g, b) = sample_to_rgb(&data[base..base + channels], channels, color);
            let o = (oy * ow + ox) * 3;
            out[o]     = r;
            out[o + 1] = g;
            out[o + 2] = b;
        }
    }

    let buf = ImageBuffer::<Rgb<u8>, _>::from_raw(ow as u32, oh as u32, out)
        .ok_or_else(|| PictoriaError::Fatal("TIFF output buffer mismatch".into()))?;
    Ok(DynamicImage::ImageRgb8(buf))
}

/// Map one interleaved TIFF sample to RGB based on the file's colour type.
fn sample_to_rgb(s: &[u8], channels: usize, color: tiff::ColorType) -> (u8, u8, u8) {
    use tiff::ColorType;
    match color {
        // Standard CMYK (stored 0 = no ink): R = (255-C)(255-K)/255, etc.
        ColorType::CMYK(_) if channels >= 4 => {
            let (c, m, y, k) = (s[0] as u16, s[1] as u16, s[2] as u16, s[3] as u16);
            let kk = 255 - k;
            ((((255 - c) * kk) / 255) as u8,
             (((255 - m) * kk) / 255) as u8,
             (((255 - y) * kk) / 255) as u8)
        }
        _ if channels >= 3 => (s[0], s[1], s[2]),
        _                  => (s[0], s[0], s[0]),
    }
}

/// Resize so the longest edge is at most `target_max`, preserving aspect ratio.
fn downscale(img: DynamicImage, target_max: u32) -> DynamicImage {
    let (w, h) = (img.width(), img.height());
    if w.max(h) > target_max {
        let factor = (w.max(h) / target_max).max(1);
        let nw     = (w / factor).max(1);
        let nh     = (h / factor).max(1);
        img.resize(nw, nh, image::imageops::FilterType::Triangle)
    } else {
        img
    }
}

fn tiff_result_to_dynamic(
    result: tiff::decoder::DecodingResult,
    w: u32,
    h: u32,
) -> Result<DynamicImage> {
    use image::{ImageBuffer, Rgb};
    use tiff::decoder::DecodingResult;

    match result {
        DecodingResult::U8(data) => {
            let channels = data.len() / (w * h) as usize;
            if channels >= 3 {
                let buf: Vec<u8> = data
                    .chunks(channels)
                    .flat_map(|c| [c[0], c[1], c[2]])
                    .collect();
                Ok(DynamicImage::ImageRgb8(
                    ImageBuffer::<Rgb<u8>, _>::from_raw(w, h, buf)
                        .ok_or_else(|| PictoriaError::Fatal("TIFF buffer size mismatch".into()))?,
                ))
            } else {
                let buf: Vec<u8> = data.iter().flat_map(|&v| [v, v, v]).collect();
                Ok(DynamicImage::ImageRgb8(
                    ImageBuffer::<Rgb<u8>, _>::from_raw(w, h, buf)
                        .ok_or_else(|| PictoriaError::Fatal("TIFF buffer size mismatch".into()))?,
                ))
            }
        }
        DecodingResult::U16(data) => {
            // Normalise 16-bit to 8-bit.
            let channels = data.len() / (w * h) as usize;
            let buf: Vec<u8> = if channels >= 3 {
                data.chunks(channels)
                    .flat_map(|c| {
                        [(c[0] >> 8) as u8, (c[1] >> 8) as u8, (c[2] >> 8) as u8]
                    })
                    .collect()
            } else {
                data.iter().flat_map(|&v| {
                    let b = (v >> 8) as u8;
                    [b, b, b]
                })
                .collect()
            };
            Ok(DynamicImage::ImageRgb8(
                ImageBuffer::<Rgb<u8>, _>::from_raw(w, h, buf)
                    .ok_or_else(|| PictoriaError::Fatal("TIFF buffer mismatch".into()))?,
            ))
        }
        _ => Err(PictoriaError::Fatal("Unsupported TIFF pixel format".into())),
    }
}

// ── PSD / PSB ─────────────────────────────────────────────────────────────

/// Extract the embedded JPEG thumbnail from a PSD/PSB resource section.
/// This avoids decompressing and compositing the full layer stack, which can
/// be extremely slow for large design files.
///
/// The PSD/PSB resource section contains blocks identified by a 2-byte
/// resource ID.  IDs 1033 and 1036 hold a JPEG thumbnail prefixed by a
/// 28-byte header (format, width, height, etc.).
fn load_psd_psb(path: &Path, target_max: u32) -> Result<DynamicImage> {
    // When indexing for region slicing, the embedded thumbnail is not good
    // enough — Photoshop stores it at a couple of hundred pixels, which leaves
    // nothing to slice.  Ask the OS to composite the layer stack instead; it is
    // colour-managed and streams, and it is the only way to get real resolution
    // out of a PSD/PSB short of parsing the merged image plane ourselves.
    // Expensive on big files, so it is confined to the detail path.
    #[cfg(target_os = "macos")]
    if target_max > TIFF_TARGET_MAX {
        if let Ok(img) = load_via_sips(path, target_max) {
            return Ok(img);
        }
        log::debug!("[psd] sips composite failed for {path:?}; using embedded preview");
    }

    match try_psb_thumbnail(path) {
        Ok(img) => return Ok(img),
        Err(e)  => log::debug!("PSB thumbnail fallback: {e}"),
    }
    // The `image` crate cannot decode the PSD/PSB layer stack, so when there is
    // no embedded preview there is nothing for us to index.  Surface an
    // actionable reason rather than the crate's cryptic "format could not be
    // determined" so the user knows how to fix it.
    load_standard(path).map_err(|_| {
        PictoriaError::Fatal(
            "No embedded preview in this PSD/PSB. Re-save it from Photoshop with \
             'Maximize Compatibility' turned on so a preview thumbnail is stored."
                .into(),
        )
    })
}

fn try_psb_thumbnail(path: &Path) -> Result<DynamicImage> {
    // Read the header and the image-resources section only.  A PSB is routinely
    // several gigabytes and the preview lives near the *start* of the file, so
    // reading the whole thing into memory (as this used to) meant a
    // multi-gigabyte allocation just to reach a thumbnail — and with NUM_WORKERS
    // files pre-processed in parallel, several of those at once.
    let f = std::fs::File::open(path)?;
    let mut r = BufReader::new(f);

    // Signature "8BPS"
    let mut magic = [0u8; 4];
    r.read_exact(&mut magic)?;
    if &magic != b"8BPS" {
        return Err(PictoriaError::Fatal("Not a PSD/PSB file".into()));
    }

    // Skip: version(2) + reserved(6) + channels(2) + height(4) + width(4) +
    //       depth(2) + color_mode(2) = 22 bytes
    r.seek(SeekFrom::Current(22))?;

    let mut len_buf = [0u8; 4];

    // Color-mode data section
    r.read_exact(&mut len_buf)?;
    r.seek(SeekFrom::Current(u32::from_be_bytes(len_buf) as i64))?;

    // Image resources section — metadata only (previews, paths, slices), so a
    // huge value here means a malformed file rather than a real section.  Cap
    // the allocation instead of trusting it.
    r.read_exact(&mut len_buf)?;
    let resources_len = u32::from_be_bytes(len_buf) as usize;
    const MAX_RESOURCES_LEN: usize = 64 * 1024 * 1024;
    if resources_len == 0 || resources_len > MAX_RESOURCES_LEN {
        return Err(PictoriaError::Fatal(
            "PSD/PSB image-resources section missing or implausibly large".into(),
        ));
    }

    // take() rather than read_exact so a truncated file still yields whatever
    // resources it does have — the old code clamped to the file length here.
    let mut resources = Vec::with_capacity(resources_len.min(1024 * 1024));
    r.take(resources_len as u64).read_to_end(&mut resources)?;

    parse_8bim_jpeg_preview(&resources)
        .map_err(|_| PictoriaError::Fatal("No JPEG thumbnail resource found in PSD/PSB".into()))
}

// ── Binary reading helpers ────────────────────────────────────────────────

fn read_u8(cur: &mut Cursor<&[u8]>) -> Result<u8> {
    let mut buf = [0u8; 1];
    cur.read_exact(&mut buf)?;
    Ok(buf[0])
}

fn read_u16_be(cur: &mut Cursor<&[u8]>) -> Result<u16> {
    let mut buf = [0u8; 2];
    cur.read_exact(&mut buf)?;
    Ok(u16::from_be_bytes(buf))
}

fn read_u32_be(cur: &mut Cursor<&[u8]>) -> Result<u32> {
    let mut buf = [0u8; 4];
    cur.read_exact(&mut buf)?;
    Ok(u32::from_be_bytes(buf))
}
