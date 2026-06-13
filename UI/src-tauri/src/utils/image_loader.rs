//! Smart image loading that handles every format Pictoria indexes:
//! JPEG, PNG, TIFF (including multi-page), PSD, and PSB.
//!
//! All functions return a decoded `image::DynamicImage` in RGB8 colour space,
//! ready for the CLIP pre-processing pipeline.

use crate::error::{Result, PictoriaError};
use image::{DynamicImage, ImageReader};
use std::{
    io::{Cursor, Read, Seek, SeekFrom},
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
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let primary = match ext.as_str() {
        "psd" | "psb" => load_psd_psb(path),
        "tif" | "tiff" => load_tiff(path),
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
            if let Ok(img) = load_via_sips(path) {
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
fn load_via_sips(path: &Path) -> Result<DynamicImage> {
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
        .args(["--resampleHeightWidthMax", "512"])
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

fn load_tiff(path: &Path) -> Result<DynamicImage> {
    // CMYK TIFFs are device-dependent and need ICC colour management to convert
    // accurately.  Our in-Rust decode applies only a naive CMYK→RGB formula,
    // which casts neutral marbles green/pink/yellow.  On macOS, sips is
    // colour-managed (ColorSync) and cheap, so prefer it for TIFFs.
    #[cfg(target_os = "macos")]
    if let Ok(img) = load_via_sips(path) {
        return Ok(img);
    }

    // Non-macOS, or sips unavailable: embedded IFD1 thumbnail, then full decode.
    if let Ok(thumb) = try_tiff_thumbnail(path) {
        return Ok(thumb);
    }
    load_tiff_main(path)
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

/// Down-scaled longest edge fed toward the CLIP pipeline (which finally wants
/// 224px).  Sampling to this size straight out of the decoded TIFF buffer avoids
/// allocating a second full-resolution image.
const TIFF_TARGET_MAX: u32 = 512;

fn load_tiff_main(path: &Path) -> Result<DynamicImage> {
    // Primary: decode with the `tiff` crate directly so we can lift its 256 MB
    // buffer cap (large multi-channel design TIFFs — e.g. 8008x15709 CMYK ≈
    // 480 MB — were otherwise rejected with "Memory limit exceeded") and convert
    // CMYK ourselves.  Fall back to the image crate for compressions the `tiff`
    // crate can't handle (e.g. some JPEG-in-TIFF variants).
    match load_tiff_via_tiff_crate(path) {
        Ok(img) => Ok(img),
        Err(e)  => {
            log::debug!("[tiff] direct decode failed ({e}); falling back to image crate");
            let img = ImageReader::open(path)?.with_guessed_format()?.decode()?;
            Ok(downscale(img, TIFF_TARGET_MAX))
        }
    }
}

fn load_tiff_via_tiff_crate(path: &Path) -> Result<DynamicImage> {
    use tiff::decoder::{Decoder, Limits};

    let file = std::fs::File::open(path)?;
    let mut decoder = Decoder::new(file)
        .map_err(|e| PictoriaError::Fatal(format!("TIFF open failed: {e}")))?
        .with_limits(Limits::unlimited());

    let (w, h) = decoder
        .dimensions()
        .map_err(|e| PictoriaError::Fatal(format!("TIFF dimensions failed: {e}")))?;
    let color = decoder
        .colortype()
        .map_err(|e| PictoriaError::Fatal(format!("TIFF colortype failed: {e}")))?;
    let result = decoder
        .read_image()
        .map_err(|e| PictoriaError::Fatal(format!("TIFF read failed: {e}")))?;

    tiff_to_downscaled_rgb(result, w, h, color, TIFF_TARGET_MAX)
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
fn load_psd_psb(path: &Path) -> Result<DynamicImage> {
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
    let data = std::fs::read(path)?;
    let mut cur = Cursor::new(data.as_slice());

    // Signature "8BPS"
    let mut magic = [0u8; 4];
    cur.read_exact(&mut magic)?;
    if &magic != b"8BPS" {
        return Err(PictoriaError::Fatal("Not a PSD/PSB file".into()));
    }

    // Skip: version(2) + reserved(6) + channels(2) + height(4) + width(4) +
    //       depth(2) + color_mode(2) = 22 bytes
    cur.seek(SeekFrom::Current(22))?;

    // Color-mode data section
    let color_mode_len = read_u32_be(&mut cur)?;
    cur.seek(SeekFrom::Current(color_mode_len as i64))?;

    // Image resources section
    let resources_len = read_u32_be(&mut cur)? as u64;
    let resources_end = cur.position() + resources_len;

    while cur.position() < resources_end {
        let mut bim = [0u8; 4];
        if cur.read_exact(&mut bim).is_err() { break; }
        if &bim != b"8BIM" { break; }

        let res_id   = read_u16_be(&mut cur)?;
        let name_len = read_u8(&mut cur)? as u64;
        // Name is padded to an even total length (name_len byte + name_len bytes).
        let pad = if (name_len + 1) % 2 != 0 { 1u64 } else { 0u64 };
        cur.seek(SeekFrom::Current((name_len + pad) as i64))?;

        let data_len = read_u32_be(&mut cur)? as u64;
        let data_end = cur.position() + data_len + (data_len % 2);

        // Resource IDs 1033 and 1036 contain JPEG thumbnail data.
        if res_id == 1033 || res_id == 1036 {
            // 28-byte thumbnail header: format(4) + width(4) + height(4) +
            //   widthbytes(4) + total_size(4) + compressedsize(4) + bpp(2) + planes(2)
            if data_len > 28 {
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
        }

        cur.seek(SeekFrom::Start(data_end))?;
    }

    Err(PictoriaError::Fatal("No JPEG thumbnail resource found in PSD/PSB".into()))
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
