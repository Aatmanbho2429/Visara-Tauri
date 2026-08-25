//! On-demand image cache for the Browse grid.
//!
//! Cached renders live in `~/.pictoria/thumbs/<hash>_v<ver>_<max>.jpg`.  The key
//! is (source path, version, size), so re-use is instant.  Generation reuses the
//! main image loader, so exotic files (CMYK TIFF, PSB) flow through the same
//! colour-managed `sips` path used during indexing.

use crate::{config, error::Result, utils::image_loader};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Grid thumbnail size (longest edge).
const THUMB_MAX: u32 = 256;

/// Bump when the decode pipeline changes so stale renders regenerate instead of
/// being served from cache (e.g. the TIFF colour-management fix).
const THUMB_VERSION: u32 = 2;

fn thumbs_dir() -> PathBuf {
    config::DATA_DIR.join("thumbs")
}

fn sized_path(orig: &Path, max: u32) -> PathBuf {
    let mut hasher = Sha256::new();
    hasher.update(orig.to_string_lossy().as_bytes());
    let hex = format!("{:x}", hasher.finalize());
    thumbs_dir().join(format!("{}_v{}_{}.jpg", &hex[..32], THUMB_VERSION, max))
}

/// Grid thumbnail (256 px).
pub fn ensure_thumb(orig: &Path) -> Result<PathBuf> {
    ensure_sized(orig, THUMB_MAX)
}

/// Return the cached render at `max` px, generating it first if needed.
pub fn ensure_sized(orig: &Path, max: u32) -> Result<PathBuf> {
    let dest = sized_path(orig, max);
    if dest.exists() {
        return Ok(dest);
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let img = image_loader::load_image(orig)?;
    let (w, h) = (img.width(), img.height());
    let out = if w.max(h) > max {
        let factor = (w.max(h) / max).max(1);
        img.resize((w / factor).max(1), (h / factor).max(1), image::imageops::FilterType::Triangle)
    } else {
        img
    };

    out.to_rgb8()
        .save_with_format(&dest, image::ImageFormat::Jpeg)
        .map_err(|e| crate::error::PictoriaError::Fatal(format!("render save failed: {e}")))?;

    Ok(dest)
}
