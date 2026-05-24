//! Smart image loading that handles every format Visara indexes:
//! JPEG, PNG, TIFF (including multi-page), PSD, and PSB.
//!
//! All functions return a decoded `image::DynamicImage` in RGB8 colour space,
//! ready for the CLIP pre-processing pipeline.

use crate::error::{Result, VisaraError};
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

    match ext.as_str() {
        "psd" | "psb" => load_psd_psb(path),
        "tif" | "tiff" => load_tiff(path),
        _ => load_standard(path),
    }
}

// ── Standard formats ──────────────────────────────────────────────────────

fn load_standard(path: &Path) -> Result<DynamicImage> {
    Ok(ImageReader::open(path)?.with_guessed_format()?.decode()?)
}

// ── TIFF ──────────────────────────────────────────────────────────────────

fn load_tiff(path: &Path) -> Result<DynamicImage> {
    // Try the embedded thumbnail in IFD1 first (present in camera / scanner TIFFs).
    if let Ok(thumb) = try_tiff_thumbnail(path) {
        return Ok(thumb);
    }
    load_tiff_main(path)
}

fn try_tiff_thumbnail(path: &Path) -> Result<DynamicImage> {
    use tiff::decoder::Decoder;

    let file = std::fs::File::open(path)?;
    let mut decoder = Decoder::new(file).map_err(|e| {
        VisaraError::Fatal(format!("TIFF decoder init failed: {e}"))
    })?;

    // IFD0 is the main image; IFD1 is the thumbnail when present.
    if !decoder.more_images() {
        return Err(VisaraError::Fatal("No IFD1 thumbnail".into()));
    }
    decoder.next_image().map_err(|e| VisaraError::Fatal(e.to_string()))?;

    let (w, h) = decoder.dimensions().map_err(|e| VisaraError::Fatal(e.to_string()))?;
    if w.max(h) > 2048 {
        return Err(VisaraError::Fatal("IFD1 too large".into()));
    }

    let result = decoder.read_image().map_err(|e| VisaraError::Fatal(e.to_string()))?;
    tiff_result_to_dynamic(result, w as u32, h as u32)
}

fn load_tiff_main(path: &Path) -> Result<DynamicImage> {
    // Use the `image` crate for the main image.  For JPEG-compressed TIFFs
    // this is already fast; for LZW/uncompressed we must read all bytes.
    let img = ImageReader::open(path)?.with_guessed_format()?.decode()?;

    // Limit size fed into the CLIP pipeline — we only need 224x224 ultimately.
    let (w, h) = (img.width(), img.height());
    if w.max(h) > 1024 {
        let factor = w.max(h) / 512;
        let nw     = (w / factor).max(1);
        let nh     = (h / factor).max(1);
        return Ok(img.resize(nw, nh, image::imageops::FilterType::Triangle));
    }
    Ok(img)
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
                        .ok_or_else(|| VisaraError::Fatal("TIFF buffer size mismatch".into()))?,
                ))
            } else {
                let buf: Vec<u8> = data.iter().flat_map(|&v| [v, v, v]).collect();
                Ok(DynamicImage::ImageRgb8(
                    ImageBuffer::<Rgb<u8>, _>::from_raw(w, h, buf)
                        .ok_or_else(|| VisaraError::Fatal("TIFF buffer size mismatch".into()))?,
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
                    .ok_or_else(|| VisaraError::Fatal("TIFF buffer mismatch".into()))?,
            ))
        }
        _ => Err(VisaraError::Fatal("Unsupported TIFF pixel format".into())),
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
    // Fallback: let the image crate try to decode the PSD composite.
    load_standard(path)
}

fn try_psb_thumbnail(path: &Path) -> Result<DynamicImage> {
    let data = std::fs::read(path)?;
    let mut cur = Cursor::new(data.as_slice());

    // Signature "8BPS"
    let mut magic = [0u8; 4];
    cur.read_exact(&mut magic)?;
    if &magic != b"8BPS" {
        return Err(VisaraError::Fatal("Not a PSD/PSB file".into()));
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

    Err(VisaraError::Fatal("No JPEG thumbnail resource found in PSD/PSB".into()))
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
