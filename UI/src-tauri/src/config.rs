//! Central configuration — all paths, constants, and tuning knobs live here.
//! Nothing is hard-coded elsewhere; other modules `use crate::config::*`.

use std::path::PathBuf;
use once_cell::sync::Lazy;

// ── App identity ───────────────────────────────────────────────────────────

pub const APP_VERSION:   &str = "1.1.7";
pub const SUPABASE_EDGE: &str =
    "https://qpxvwdxuhgbthzbcppye.supabase.co/functions/v1";

// ── Embedding model ────────────────────────────────────────────────────────

/// CLIP ViT-B/32 embedding dimension.
pub const EMB_DIM: usize = 768;

/// ImageNet mean / std used for CLIP pre-processing (RGB order).
pub const CLIP_MEAN: [f32; 3] = [0.48145466, 0.4578275, 0.40821073];
pub const CLIP_STD:  [f32; 3] = [0.26862954, 0.26130258, 0.27577711];

/// Input resolution expected by the CLIP ViT-B/32 image encoder.
pub const CLIP_INPUT_SIZE: u32 = 224;

// ── Processing ────────────────────────────────────────────────────────────

/// Images processed per ONNX inference call.
pub const BATCH_SIZE: usize = 32;

/// Rayon thread-pool size for parallel image pre-processing.
pub const NUM_WORKERS: usize = 8;

/// Number of bytes read from the start of each file for the fast hash.
pub const HASH_BYTES: u64 = 65_536; // 64 KiB

// ── Supported image extensions ────────────────────────────────────────────

pub const IMAGE_EXTENSIONS: &[&str] =
    &["jpg", "jpeg", "png", "tif", "tiff", "psd", "psb"];

// ── Filesystem paths (resolved once at startup) ───────────────────────────

/// Directory that holds the encrypted ONNX model bundled with the app.
/// In development this is `src-tauri/`; in a packaged bundle it is the
/// resource directory Tauri sets up next to the executable.
pub static MODELS_DIR: Lazy<PathBuf> = Lazy::new(|| {
    // Packaged build: Tauri sets TAURI_RESOURCE_DIR to the bundle resource dir.
    if let Ok(res) = std::env::var("TAURI_RESOURCE_DIR") {
        return PathBuf::from(res).join("models");
    }
    // Development: CARGO_MANIFEST_DIR = UI/src-tauri
    // The encrypted model lives in python/models/ at the project root.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..") // UI/
        .join("..") // project root
        .join("python")
        .join("models")
});

/// Encrypted CLIP model file shipped with every release.
pub static MODEL_ENC_PATH: Lazy<PathBuf> =
    Lazy::new(|| MODELS_DIR.join("clip_vitb32.onnx.enc"));

/// User-scoped data directory:  ~/.visara/   (created on first run).
pub static DATA_DIR: Lazy<PathBuf> = Lazy::new(|| {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".visara")
});

/// SQLite database that maps file paths ↔ vector IDs.
pub static DB_PATH: Lazy<PathBuf> = Lazy::new(|| DATA_DIR.join("meta.db"));

/// Custom flat-vector store (replaces FAISS index).
pub static VECTOR_STORE_PATH: Lazy<PathBuf> =
    Lazy::new(|| DATA_DIR.join("vectors.bin"));

/// Bearer-token persisted between sessions.
/// This is the ONLY file written to disk — deleted on logout.
/// The model key and user data live in memory only (sourced from validate-token).
pub static TOKEN_FILE: Lazy<PathBuf> =
    Lazy::new(|| dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".visara_token"));
