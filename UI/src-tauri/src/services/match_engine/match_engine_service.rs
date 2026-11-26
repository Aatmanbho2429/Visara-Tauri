// Background upkeep for the exact-match engine: fingerprint (and later feature) backfills.

use crate::{
    config::{BACKFILL_STARTUP_WAIT_SECS, SIDECAR_HEALTH_POLL_MS},
    core::{database, progress, sidecar},
    services::sync,
};
use std::{sync::Mutex, time::Duration};

// Serialises backfills so the post-sync trigger and the startup trigger never fingerprint the same files twice.
static BACKFILL_LOCK: Mutex<()> = Mutex::new(());

// Runs every backfill for one folder (blocking); call from a spawned thread.
pub fn backfill_folder(folder: &str) {
    let _guard = BACKFILL_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    sync::backfill_fingerprints(folder);
}

// At launch, once the sidecar is up, backfills every watched folder regardless of whether a sync runs.
pub fn spawn_startup_backfill() {
    std::thread::spawn(|| {
        let deadline = std::time::Instant::now() + Duration::from_secs(BACKFILL_STARTUP_WAIT_SECS);
        while !sidecar::is_healthy() {
            if std::time::Instant::now() >= deadline {
                log::info!("[match_engine] startup backfill: sidecar never came up, skipping this launch");
                return;
            }
            std::thread::sleep(Duration::from_millis(SIDECAR_HEALTH_POLL_MS));
        }
        let folders = match database::open().and_then(|c| database::watched_folder_paths(&c)) {
            Ok(f) => f,
            Err(e) => {
                log::warn!("[match_engine] startup backfill: cannot list folders: {e}");
                return;
            }
        };
        for folder in folders {
            // A running sync holds the DB write lock for seconds at a time, so let it finish first.
            let sync_deadline = std::time::Instant::now() + Duration::from_secs(BACKFILL_STARTUP_WAIT_SECS);
            while progress::get_progress().active && std::time::Instant::now() < sync_deadline {
                std::thread::sleep(Duration::from_secs(2));
            }
            backfill_folder(&folder);
        }
    });
}
