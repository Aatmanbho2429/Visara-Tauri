//! Central configuration — all paths, constants, and tuning knobs live here.
//! Nothing is hard-coded elsewhere; other modules `use crate::config::*`.

use std::path::PathBuf;
use once_cell::sync::Lazy;

// ── App identity ───────────────────────────────────────────────────────────

pub const APP_VERSION:   &str = "1.1.8";
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
/// Set at startup from `app.path().resource_dir()` (packaged build) or
/// falls back to the in-repo dev path when running `cargo tauri dev`.
static MODELS_DIR: once_cell::sync::OnceCell<PathBuf> =
    once_cell::sync::OnceCell::new();

/// Called from `setup()` once the Tauri `AppHandle` is available.
pub fn set_resource_dir(resource_dir: PathBuf) {
    let dir = resource_dir.join("models");
    let _ = MODELS_DIR.set(dir);
}

/// Resolve the encrypted CLIP model location.
/// Packaged build → `<resource_dir>/models/clip_vitb32.onnx.enc`
/// Dev build      → `<repo>/python/models/clip_vitb32.onnx.enc`
pub fn model_enc_path() -> PathBuf {
    if let Some(dir) = MODELS_DIR.get() {
        return dir.join("clip_vitb32.onnx.enc");
    }
    // Dev fallback: CARGO_MANIFEST_DIR = UI/src-tauri
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..") // UI/
        .join("..") // project root
        .join("python")
        .join("models")
        .join("clip_vitb32.onnx.enc")
}

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
