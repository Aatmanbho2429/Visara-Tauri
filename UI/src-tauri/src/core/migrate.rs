//! One-time embedding-schema migration.
//!
//! When the way vectors are produced changes (preprocessing, the colour vector,
//! or the CLIP model), every stored vector becomes invalid and the whole library
//! must be re-indexed.  We detect this on startup by comparing a persisted
//! schema version (SQLite `PRAGMA user_version`) against [`EMBED_SCHEMA_VERSION`].
//!
//! On a bump we:
//!   * delete `vectors.bin` (its format/contents are stale), and
//!   * drop a `.reembed_pending` marker file.
//!
//! The marker — not just an in-memory flag — is what makes the re-index
//! crash-safe: if the app quits mid-reindex the marker is still present next
//! launch and the re-index resumes.  It is cleared only after a full reconcile
//! pass completes (see `core::watcher::reconcile_all`).
//!
//! Crucially we DO NOT delete rows from `files`: tags now cascade off
//! `files(id)`, so wiping `files` would wipe Browse's tags.  Instead the vectors
//! are rebuilt *in place* against the existing `faiss_id`s
//! (see `services::sync::reembed_folder`).

use crate::{
    config::{DATA_DIR, DB_PATH, VECTOR_STORE_PATH},
    core::database,
};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};

/// Bump this whenever embeddings change so existing installs re-index once.
/// v2: centre-crop preprocessing + dual design/colour vectors.
/// v3: grayscale CLIP input for pattern-only design vectors; weights 0.85/0.15.
/// v4: DINOv2 ViT-B/14 replaces CLIP; 1536-dim design vector; ranking is
///     pure design similarity (color stored for display only, weight 0.0).
/// v5: multi-region index — each file stores one vector per region (whole frame
///     plus overlapping windows) so a small motif can match the larger design it
///     appears inside; whole-frame pre-processing pads instead of centre-cropping
///     so no part of a non-square image is discarded.
/// v6: reverts v5's pad-to-square.  The padding gave every image of the same
///     aspect ratio an identical band artefact, which then dominated similarity —
///     a 1.98-aspect query returned twenty 1.98-aspect images regardless of
///     design, and the true parent (1.50 aspect, same marble) sat at rank 237.
///     It also put square region slices in a different visual domain from a
///     padded query, disabling partial matching entirely.  Back to the centre
///     crop; full-frame coverage comes from the region windows instead.
/// v7: DINOv2/ONNX replaced by the sidecar (Gabor-rose + Gram-matrix ranking,
///     SIFT/RANSAC verification). No more region slicing — one descriptor per
///     file; "found inside" is now a direct geometric proof instead of a
///     region-vs-whole-frame score margin.
/// v8: gram_descriptor now runs on a desaturated (grayscale) image instead of
///     RGB — the CNN's colour-sensitive normalisation was suppressing
///     same-design different-colourway matches (e.g. a yellow variant of a
///     blue tile) out of the ranking entirely. Every stored gram vector was
///     computed against colour and is no longer comparable to a freshly
///     computed grayscale one.
/// v9: DINO embeddings are back, now computed sidecar-side (ONNX under the
///     licence key, not `ort` in Rust) and stored per file alongside rose and
///     gram, which are kept. Retrieval changed shape with them: instead of a
///     fixed top-1500 rose+gram shortlist, `VectorStore::near_family` returns
///     every file clearing an embedding-cosine floor and SIFT/RANSAC verifies
///     all of them. Existing entries carry no embedding at all, so they cannot
///     be scored and must be rebuilt.
pub const EMBED_SCHEMA_VERSION: i64 = 9;

static REEMBED_PENDING: AtomicBool = AtomicBool::new(false);

fn marker_path() -> PathBuf {
    DATA_DIR.join(".reembed_pending")
}

// ── One-time full library reset ───────────────────────────────────────────
//
// Distinct from the re-embed migration above. That one keeps `files` and
// rebuilds vectors in place; this one removes the local library outright —
// `meta.db` (including the watched-folder list), `vectors.bin` and the
// thumbnail cache — so the user re-adds their folders and everything is
// rebuilt by the current pipeline from scratch.
//
// Reserved for releases where rebuilding in place is not enough. Because it
// costs the user their folder setup, it also arms a one-time notice the UI
// shows as a dismissible banner (see `reset_notice_pending`).
//
// NOTE: the auth token lives at `~/.pictoria_token` (and/or the OS keychain),
// *outside* `DATA_DIR`, so wiping the library never signs anyone out.

/// Bump to trigger another one-time wipe on the next release.
///
/// v2: the DINO embedding returned and retrieval changed shape with it (see
///     `EMBED_SCHEMA_VERSION` v9), and the catalog feature was removed. The
///     schema bump alone would have rebuilt vectors in place and kept the
///     existing `meta.db`, but that database still carries the now-orphaned
///     `catalog_themes` table and its saved themes, which nothing reads any
///     more. Wiping outright is the clean line: users come back on a database
///     this build actually created.
const LIBRARY_RESET_VERSION: u32 = 2;

fn reset_done_marker() -> PathBuf {
    DATA_DIR.join(format!(".library_reset_v{LIBRARY_RESET_VERSION}"))
}

fn reset_notice_marker() -> PathBuf {
    DATA_DIR.join(".reset_notice_pending")
}

/// Wipe the local library exactly once per `LIBRARY_RESET_VERSION`.
///
/// MUST run before anything opens `meta.db` or the vector store — call it
/// ahead of `run_startup()`, which opens the DB as its first act.
pub fn run_library_reset() {
    if reset_done_marker().exists() {
        return; // already reset for this version
    }

    // Only apologise to people who actually lost something. On a fresh
    // install there is nothing to delete, and a banner saying we removed
    // their folders would be both wrong and alarming — so the marker is
    // still written (this counts as done) but the notice is not armed.
    let had_library = DB_PATH.exists() || VECTOR_STORE_PATH.exists();

    let _ = std::fs::remove_file(DB_PATH.as_path());
    let _ = std::fs::remove_file(VECTOR_STORE_PATH.as_path());
    let _ = std::fs::remove_dir_all(DATA_DIR.join("thumbs"));
    // Stale re-embed marker would otherwise arm a re-index of a library that
    // no longer exists.
    let _ = std::fs::remove_file(marker_path());

    if let Err(e) = std::fs::create_dir_all(DATA_DIR.as_path()) {
        log::warn!("[migrate] could not recreate data dir after reset: {e}");
        return; // don't write the marker — retry on the next launch
    }

    if let Err(e) = std::fs::File::create(reset_done_marker()) {
        // Without the marker this would wipe again every launch, so treat a
        // failure here as fatal to the reset rather than pressing on.
        log::warn!("[migrate] could not write reset marker: {e}");
        return;
    }

    if had_library {
        let _ = std::fs::File::create(reset_notice_marker());
    }

    log::info!(
        "[migrate] library reset v{LIBRARY_RESET_VERSION} complete (had_library={had_library})"
    );
}

/// True while the post-reset banner still needs showing. Backed by a file, not
/// a flag, so quitting before dismissing it doesn't swallow the message.
pub fn reset_notice_pending() -> bool {
    reset_notice_marker().exists()
}

/// Called when the user closes the banner.
pub fn clear_reset_notice() {
    if let Err(e) = std::fs::remove_file(reset_notice_marker()) {
        if e.kind() != std::io::ErrorKind::NotFound {
            log::warn!("[migrate] could not clear reset notice: {e}");
        }
    }
}

/// Run once at startup, before the watcher begins reconciling.  Performs the
/// `file_tags` schema migration and, when the embedding schema has changed, arms
/// a full re-index.
pub fn run_startup() {
    let con = match database::open() {
        Ok(c) => c,
        Err(e) => { log::warn!("[migrate] cannot open DB: {e}"); return; }
    };

    // Normalise the tags table to reference files(id) with ON DELETE CASCADE.
    if let Err(e) = database::migrate_file_tags_to_file_id(&con) {
        log::warn!("[migrate] file_tags migration failed: {e}");
    }

    let current = database::user_version(&con).unwrap_or(0);
    if current < EMBED_SCHEMA_VERSION {
        log::info!(
            "[migrate] embedding schema {current} -> {EMBED_SCHEMA_VERSION}; scheduling re-index"
        );
        // The old vector file's format/embeddings no longer apply.
        let _ = std::fs::remove_file(VECTOR_STORE_PATH.as_path());
        let _ = std::fs::File::create(marker_path());
        let _ = database::set_user_version(&con, EMBED_SCHEMA_VERSION);
    }

    if marker_path().exists() {
        REEMBED_PENDING.store(true, Ordering::SeqCst);
        log::info!("[migrate] re-index pending — folders will be re-embedded once the model loads");
    }
}

/// True while a full re-index is outstanding.  `core::watcher` consults this to
/// rebuild vectors in place for already-indexed files before the normal sync.
pub fn reembed_pending() -> bool {
    REEMBED_PENDING.load(Ordering::SeqCst)
}

/// Clear the pending state after a full reconcile pass has re-embedded every
/// folder.  Removes the on-disk marker so future launches don't re-index.
pub fn clear_reembed_pending() {
    if REEMBED_PENDING.swap(false, Ordering::SeqCst) {
        let _ = std::fs::remove_file(marker_path());
        log::info!("[migrate] re-index complete — marker cleared");
    }
}
