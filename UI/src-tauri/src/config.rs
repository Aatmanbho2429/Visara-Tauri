//! Central configuration — all paths, constants, and tuning knobs live here.
//! Nothing is hard-coded elsewhere; other modules `use crate::config::*`.

use std::path::PathBuf;
use once_cell::sync::Lazy;

// ── App identity ───────────────────────────────────────────────────────────

pub const APP_VERSION:   &str = "1.1.28";
pub const SUPABASE_EDGE: &str =
    "https://qpxvwdxuhgbthzbcppye.supabase.co/functions/v1";

// ── Embedding model ────────────────────────────────────────────────────────

/// DINOv2 ViT-B/14 embedding dimension: 768-dim CLS token + 768-dim patch mean.
pub const EMB_DIM: usize = 1536;

/// Standard ImageNet mean / std used for DINOv2 pre-processing (RGB order).
pub const CLIP_MEAN: [f32; 3] = [0.485, 0.456, 0.406];
pub const CLIP_STD:  [f32; 3] = [0.229, 0.224, 0.225];

/// Input resolution expected by the DINOv2 ViT-B/14 image encoder.
pub const CLIP_INPUT_SIZE: u32 = 224;

// ── Processing ────────────────────────────────────────────────────────────

/// Images processed per ONNX inference call.
pub const BATCH_SIZE: usize = 32;

/// Rayon thread-pool size for parallel image pre-processing.
pub const NUM_WORKERS: usize = 8;

/// Number of bytes read from the start of each file for the fast hash.
pub const HASH_BYTES: u64 = 65_536; // 64 KiB

/// How often the background watcher streams a progress snapshot to the UI
/// while a folder is being indexed.
pub const PROGRESS_EMIT_INTERVAL_MS: u64 = 400;

// ── Licensing ─────────────────────────────────────────────────────────────

/// How long the app may run on a cached "valid" subscription state without
/// being able to reach Supabase (e.g. no internet) before it forces a fresh
/// `validate-token-test` round-trip and unloads the AI model if that fails
/// too. Keeps the app usable offline for short periods (flights, poor
/// connectivity) without allowing an indefinitely-cached "expired but still
/// works" session.
pub const OFFLINE_GRACE_SECS: i64 = 3 * 24 * 3600; // 3 days

// ── Supported image extensions ────────────────────────────────────────────

pub const IMAGE_EXTENSIONS: &[&str] =
    &["jpg", "jpeg", "png", "tif", "tiff", "psd", "psb"];

// ── Filesystem paths (resolved once at startup) ───────────────────────────

/// Resource directory resolved at startup from `app.path().resource_dir()`.
/// Layout differs between platforms and dev/packaged builds, so we never assume
/// a single sub-path — `model_enc_path()` probes several candidates below.
static RESOURCE_DIR: once_cell::sync::OnceCell<PathBuf> =
    once_cell::sync::OnceCell::new();

/// Called from `setup()` once the Tauri `AppHandle` is available.
pub fn set_resource_dir(resource_dir: PathBuf) {
    let _ = RESOURCE_DIR.set(resource_dir);
}

/// Encrypted model file name.
const MODEL_FILE: &str = "clip_vitb32.onnx.enc";

/// Resolve the encrypted CLIP model by probing every location it could live in
/// across platforms and build modes, returning the first that exists:
///   1. `<resource_dir>/models/<file>`  — packaged bundle layout
///   2. `<resource_dir>/<file>`         — flat layout (Tauri dev copies here)
///   3. `<crate>/resources/models/<file>` — in-repo resources (dev)
///   4. `<repo>/python/models/<file>`     — legacy in-repo model (dev)
///
/// macOS dev resolved `resource_dir` to `target/debug/`, where Tauri places the
/// model flat (no `models/` sub-dir) — candidate 2 covers that case, which the
/// old single-path logic missed (it only checked candidate 1).
pub fn model_enc_path() -> PathBuf {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Some(dir) = RESOURCE_DIR.get() {
        candidates.push(dir.join("models").join(MODEL_FILE));
        candidates.push(dir.join(MODEL_FILE));
    }

    // Dev fallbacks relative to the crate (CARGO_MANIFEST_DIR = UI/src-tauri).
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    candidates.push(manifest.join("resources").join("models").join(MODEL_FILE));
    candidates.push(
        manifest
            .join("..") // UI/
            .join("..") // project root
            .join("python")
            .join("models")
            .join(MODEL_FILE),
    );

    for candidate in &candidates {
        if candidate.exists() {
            log::info!("[config] model file resolved to {:?}", candidate);
            return candidate.clone();
        }
    }

    log::warn!(
        "[config] model file '{MODEL_FILE}' not found in any known location; \
         tried {:?}",
        candidates
    );
    candidates
        .into_iter()
        .next()
        .unwrap_or_else(|| PathBuf::from(MODEL_FILE))
}

/// User-scoped data directory:  ~/.pictoria/   (created on first run).
pub static DATA_DIR: Lazy<PathBuf> = Lazy::new(|| {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".pictoria")
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
        .join(".pictoria_token"));
