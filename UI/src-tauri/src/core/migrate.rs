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
    config::{DATA_DIR, VECTOR_STORE_PATH},
    core::database,
};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};

/// Bump this whenever embeddings change so existing installs re-index once.
/// v2: centre-crop preprocessing + dual design/colour vectors.
pub const EMBED_SCHEMA_VERSION: i64 = 2;

static REEMBED_PENDING: AtomicBool = AtomicBool::new(false);

fn marker_path() -> PathBuf {
    DATA_DIR.join(".reembed_pending")
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
