// Central configuration — all paths, constants, and tuning knobs live here.
// Nothing is hard-coded elsewhere; other modules `use crate::config::*`.

use std::path::PathBuf;
use once_cell::sync::Lazy;

// ── App identity ───────────────────────────────────────────────────────────

pub const APP_VERSION:   &str = "1.1.40";
pub const SUPABASE_EDGE: &str =
    "https://qpxvwdxuhgbthzbcppye.supabase.co/functions/v1";

// ── Design descriptor (DINO embedding + Gabor rose + Gram-matrix) ─────────

// DINO embedding width for one zoom level. MUST match the model's actual
// output — `sidecar/pipeline.py::load_embed_model` measures it and reports it
// through `/health`, and `core::sidecar` refuses a mismatch rather than let a
// wrong stride be written into `vectors.bin`.
pub const EMBED_DIM: usize = 1536;

// Zoom levels the embedding is computed at, matching `_GRAM_ZOOM_SCALES` in
// `sidecar/pipeline.py` — the crops are literally shared between the two.
pub const EMBED_ZOOM_LEVELS: usize = 3;

// Cosine floor for the near family: every file scoring at least this against
// the query goes to SIFT/RANSAC verification, however many that is. This
// replaced a fixed top-N shortlist, so it is the only thing bounding how much
// work a search does — see `services::search`.
//
// NOT yet calibrated against the real model. Raw cosine baselines are
// model-specific: on a grayscale-only corpus, unrelated pairs commonly sit in
// the 0.4-0.6 range, so 0.70 may prove looser than "70% similar" sounds. The
// number to watch is `near_family_n` in the search timing log — if it is a
// large fraction of the library on an ordinary query, this is too low.
pub const NEAR_FAMILY_MIN_SIM: f32 = 0.70;

// Verification stops when either bound is hit; whatever is proven by then is
// returned rather than the search hanging until every candidate is checked.
// The count cap is the safety net for the uncalibrated floor above; the time
// budget is what the user actually feels. Both are deliberately generous
// because unverified results are still shown (labelled "unchecked", not
// dropped) — see `services::search::verify_pass`.
pub const VERIFY_TIME_BUDGET_MS: u64 = 2_500;
pub const VERIFY_MAX_CANDIDATES: usize = 400;

// Logs a cosine-distribution histogram on every search (see
// `VectorStore::cosine_histogram` and the calibration procedure in
// SEARCH-LATENCY-PLAN.md Phase 3). It's a second full-population scan of the
// embedding — roughly the same cost as `near_family`'s own embedding pass —
// purely for diagnostics, so it isn't free at 50k+ files. Meant to be run
// for the 10-20 calibration searches the procedure describes, then flipped
// back to `false` once `NEAR_FAMILY_MIN_SIM` is set from real data, not left
// on permanently.
pub const CALIBRATION_LOGGING_ENABLED: bool = true;

// Gabor orientation-energy histogram: one bin per direction.
pub const ROSE_DIM: usize = 8;

// Gram-matrix texture descriptor: 3 zoom levels x 24x24 flattened channel
// correlations (mobilenet_v2 block 4).
pub const GRAM_ZOOM_LEVELS: usize = 3;
pub const GRAM_DIM_PER_ZOOM: usize = 576;

// Weights for the secondary rose+gram score. No longer selects candidates —
// the DINO embedding does that — but it is still computed, stored and logged
// alongside, and breaks ties between two files at the same embedding cosine.
pub const ROSE_WEIGHT: f32 = 0.4;
pub const GRAM_WEIGHT: f32 = 0.6;

// ── Sidecar (Python: Gabor/Gram descriptors + SIFT/RANSAC verification) ──

// Localhost port the sidecar's HTTP server listens on.
pub const SIDECAR_PORT: u16 = 8756;

// How often `core::sidecar` polls `/health` while waiting for the model to
// finish loading at startup.
pub const SIDECAR_HEALTH_POLL_MS: u64 = 300;

// ── Processing ────────────────────────────────────────────────────────────

// Rayon thread-pool size for parallel image pre-processing.
pub const NUM_WORKERS: usize = 8;

// Number of bytes read from the start of each file for the fast hash.
pub const HASH_BYTES: u64 = 65_536; // 64 KiB

// How often the background watcher streams a progress snapshot to the UI
// while a folder is being indexed.
pub const PROGRESS_EMIT_INTERVAL_MS: u64 = 400;

// ── Licensing ─────────────────────────────────────────────────────────────

// How long the app may run on a cached "valid" subscription state without
// being able to reach Supabase (e.g. no internet) before it forces a fresh
// `validate-token-test` round-trip and unloads the AI model if that fails
// too. Keeps the app usable offline for short periods (flights, poor
// connectivity) without allowing an indefinitely-cached "expired but still
// works" session.
pub const OFFLINE_GRACE_SECS: i64 = 3 * 24 * 3600; // 3 days

// ── Supported image extensions ────────────────────────────────────────────
//, "tif", "tiff", "psd", "psb"
pub const IMAGE_EXTENSIONS: &[&str] =
    &["jpg", "jpeg", "png"];

// ── Filesystem paths (resolved once at startup) ───────────────────────────

// Resource directory resolved at startup from `app.path().resource_dir()`.
// Layout differs between platforms and dev/packaged builds, so we never assume
// a single sub-path — `model_enc_path()` probes several candidates below.
static RESOURCE_DIR: once_cell::sync::OnceCell<PathBuf> =
    once_cell::sync::OnceCell::new();

// Called from `setup()` once the Tauri `AppHandle` is available.
pub fn set_resource_dir(resource_dir: PathBuf) {
    let _ = RESOURCE_DIR.set(resource_dir);
}

// Frozen sidecar executable name, per-platform (matches the PyInstaller
// output and the Tauri `externalBin` target-triple naming convention).
// `pub(crate)` so `core::sidecar` can reuse the same name when sweeping up
// an orphaned process at startup, rather than duplicating the literal.
#[cfg(target_os = "windows")]
pub(crate) const SIDECAR_FILE: &str = "pictoria-sidecar.exe";
#[cfg(not(target_os = "windows"))]
pub(crate) const SIDECAR_FILE: &str = "pictoria-sidecar";

// Resolve the frozen sidecar binary the same way `model_enc_path()` used to
// resolve the encrypted model — probe bundle layout, then dev fallbacks.
pub fn sidecar_bin_path() -> PathBuf {
    let mut candidates: Vec<PathBuf> = Vec::new();

    // `externalBin` (see tauri.conf.json) drops the sidecar next to the main
    // executable — `Pictoria.app/Contents/MacOS/` on macOS, the install dir on
    // Windows, and `target/<profile>/` under `tauri dev`. This is checked
    // first because it is where a packaged build actually puts it; the
    // resource-dir entries below only ever matched the older `bundle.resources`
    // layout and are kept so an installed copy from before that change keeps
    // working across an update.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join(SIDECAR_FILE));
        }
    }

    if let Some(dir) = RESOURCE_DIR.get() {
        candidates.push(dir.join("bin").join(SIDECAR_FILE));
        candidates.push(dir.join(SIDECAR_FILE));
    }

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // Where CI stages the frozen binary for `externalBin` to pick up. The
    // target-triple suffix Tauri requires there is stripped when it lands in
    // the bundle, so only the plain name is probed at runtime.
    candidates.push(manifest.join("binaries").join(SIDECAR_FILE));
    candidates.push(
        manifest.join("..").join("..").join("sidecar").join("dist").join(SIDECAR_FILE),
    );

    for candidate in &candidates {
        if candidate.exists() {
            log::info!("[config] sidecar binary resolved to {:?}", candidate);
            return candidate.clone();
        }
    }

    log::warn!(
        "[config] sidecar binary '{SIDECAR_FILE}' not found in any known location; \
         tried {:?}",
        candidates
    );
    candidates
        .into_iter()
        .next()
        .unwrap_or_else(|| PathBuf::from(SIDECAR_FILE))
}

// User-scoped data directory:  ~/.pictoria/   (created on first run).
pub static DATA_DIR: Lazy<PathBuf> = Lazy::new(|| {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".pictoria")
});

// SQLite database that maps file paths ↔ vector IDs.
pub static DB_PATH: Lazy<PathBuf> = Lazy::new(|| DATA_DIR.join("meta.db"));

// Custom flat-vector store (replaces FAISS index).
pub static VECTOR_STORE_PATH: Lazy<PathBuf> =
    Lazy::new(|| DATA_DIR.join("vectors.bin"));

// Bearer-token persisted between sessions.
// This is the ONLY file written to disk — deleted on logout.
// The model key and user data live in memory only (sourced from validate-token).
pub static TOKEN_FILE: Lazy<PathBuf> =
    Lazy::new(|| dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".pictoria_token"));
