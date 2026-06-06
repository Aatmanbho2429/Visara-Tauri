//! On-demand thumbnail cache for the Browse grid.
//!
//! Thumbnails live in `~/.visara/thumbs/<hash>.jpg` (256 px longest edge).  The
//! cache key is the source path, so re-browsing is instant.  Generation reuses
//! the main image loader, so exotic files (CMYK TIFF, PSB) flow through the same
//! `sips` fallback used during indexing.

use crate::{config, error::Result, utils::image_loader};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const THUMB_MAX: u32 = 256;

fn thumbs_dir() -> PathBuf {
    config::DATA_DIR.join("thumbs")
}

/// Deterministic cache path for a source image (does not create the file).
pub fn thumb_path(orig: &Path) -> PathBuf {
    let mut hasher = Sha256::new();
    hasher.update(orig.to_string_lossy().as_bytes());
    let hex = format!("{:x}", hasher.finalize());
    thumbs_dir().join(format!("{}.jpg", &hex[..32]))
}

/// Return the cached thumbnail path, generating it first if needed.
pub fn ensure_thumb(orig: &Path) -> Result<PathBuf> {
    let dest = thumb_path(orig);
    if dest.exists() {
        return Ok(dest);
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let img = image_loader::load_image(orig)?;
    let (w, h) = (img.width(), img.height());
    let thumb = if w.max(h) > THUMB_MAX {
        let factor = (w.max(h) / THUMB_MAX).max(1);
        img.resize((w / factor).max(1), (h / factor).max(1), image::imageops::FilterType::Triangle)
    } else {
        img
    };

    thumb
        .to_rgb8()
        .save_with_format(&dest, image::ImageFormat::Jpeg)
        .map_err(|e| crate::error::VisaraError::Fatal(format!("thumbnail save failed: {e}")))?;

    Ok(dest)
}
