//! File-system utilities: directory scanning and fast content hashing.

use crate::{config, error::Result};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{BufReader, Read},
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

/// Directories silently skipped during recursive image scans.
const SKIP_DIRS: &[&str] = &["__macosx", ".DS_Store", "Thumbs.db"];

// ── Directory scanning ────────────────────────────────────────────────────

/// Recursively yield every image file under `folder`.
/// Hidden / system directories are skipped.
pub fn scan_images(folder: &Path) -> Vec<PathBuf> {
    WalkDir::new(folder)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy().to_lowercase();
            !SKIP_DIRS.iter().any(|s| name == *s)
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| is_image(e.path()))
        .map(|e| {
            // Normalise separators so DB lookups are consistent.
            e.path().to_path_buf()
        })
        .collect()
}

fn is_image(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    config::IMAGE_EXTENSIONS.contains(&ext.as_str())
}

// ── File hashing ──────────────────────────────────────────────────────────

/// SHA-256 of `file_size_bytes || first_HASH_BYTES_of_file`.
///
/// Using only the file size + head bytes (64 KiB) keeps hashing sub-millisecond
/// even for gigabyte TIFFs while still reliably detecting any modification.
pub fn fast_hash(path: &Path) -> Result<String> {
    let metadata  = std::fs::metadata(path)?;
    let file_size = metadata.len();

    let file   = File::open(path)?;
    let mut rdr = BufReader::new(file);
    let mut buf = vec![0u8; config::HASH_BYTES as usize];
    let n = rdr.read(&mut buf)?;

    let mut hasher = Sha256::new();
    hasher.update(file_size.to_le_bytes());
    hasher.update(&buf[..n]);

    Ok(hex::encode(hasher.finalize()))
}
