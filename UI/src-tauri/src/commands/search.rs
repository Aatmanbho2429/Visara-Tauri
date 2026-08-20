//! Tauri command handlers for the search pipeline.
//!
//! The sidecar loads its model once at app startup, independent of login —
//! `core::sidecar::is_ready()` just needs the health check to have passed
//! AND an active session, no per-search key handoff like the old
//! license-encrypted-model flow required.
//!
//! Progress is streamed to Angular via three Tauri events:
//!   search_progress  — snapshot while running
//!   search_complete  — final results
//!   search_error     — fatal error that stopped the search

use crate::{
    core::sidecar,
    services::search,
};
use serde_json::json;
use std::{path::PathBuf, sync::Mutex, time::Duration};
use tauri::Emitter;
use tokio::time;

// ── Search state — prevents concurrent searches ───────────────────────────

struct SearchState {
    running: bool,
}

static SEARCH_STATE: std::sync::OnceLock<Mutex<SearchState>> = std::sync::OnceLock::new();

fn search_state() -> &'static Mutex<SearchState> {
    SEARCH_STATE.get_or_init(|| Mutex::new(SearchState { running: false }))
}

// ── Command ───────────────────────────────────────────────────────────────

/// Polled by the frontend to show/hide "Model is loading…" and gate the
/// Search button, since the sidecar now starts at app launch rather than
/// being loaded lazily on the first search. Follows the same
/// emit-`<command>_response` convention as every other command here.
#[tauri::command]
pub fn sidecar_status(app: tauri::AppHandle) {
    let result = json!({
        "success": true,
        "message": "ok",
        "data": {
            "healthy": sidecar::is_healthy(),
            "ready":   sidecar::is_ready(),
        }
    });
    let _ = app.emit("sidecar_status_response", result);
}

/// `scope_paths` empty or omitted → search every watched folder in the Library.
/// Otherwise the search is restricted to the provided folders.
#[tauri::command]
pub async fn start_search(
    app:         tauri::AppHandle,
    image_path:  String,
    scope_paths: Option<Vec<String>>,
    top_k:       usize,
) {
    // Guard: reject concurrent searches.
    {
        let mut state = search_state().lock().unwrap();
        if state.running {
            let _ = app.emit("search_error", json!({
                "success": false,
                "message": "A search is already in progress.",
                "data":    null
            }));
            return;
        }
        state.running = true;
    }

    if !sidecar::is_ready() {
        let _ = app.emit("search_error", json!({
            "success": false,
            "message": "Still getting ready — please wait a moment and try again.",
            "data":    null
        }));
        search_state().lock().unwrap().running = false;
        return;
    }

    // Query basename for the searching snapshot, captured before image_path is
    // moved into the worker closure below.
    let query_name = std::path::Path::new(&image_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    let app_clone = app.clone();
    // The initial "Searching" snapshot below (after spawn_blocking) also
    // needs `query_name`, so the worker closure gets its own clone rather
    // than moving the original in.
    let query_name_for_worker = query_name.clone();

    // Spawn the heavy work on a blocking thread so Tokio stays responsive.
    tokio::task::spawn_blocking(move || {
        let image = PathBuf::from(&image_path);
        let scope: Vec<PathBuf> = scope_paths
            .unwrap_or_default()
            .into_iter()
            .map(PathBuf::from)
            .collect();

        // Real progress during the verify phase — SIFT/RANSAC over a couple
        // hundred candidates is tens of seconds even parallelized
        // server-side, and a single frozen "Searching" snapshot for that
        // whole wait reads as hung. `search::execute` calls this between
        // verify chunks; `query_name` is the same basename the initial
        // snapshot below used, so the label doesn't jump when this takes over.
        // `mirrored` marks the second, flipped-query pass that only runs when
        // the first proved nothing (see `search::execute`). It gets its own
        // label because the bar restarting at zero would otherwise look like
        // the search had glitched and started over.
        let on_verify_progress = |done: usize, total: usize, mirrored: bool| {
            let percent = if total > 0 { (done as f32 / total as f32 * 100.0).min(100.0) } else { 0.0 };
            let _ = app_clone.emit("search_progress", json!({
                "progress": {
                    "active":  true,
                    "phase":   if mirrored { "Checking mirrored orientation" } else { "Verifying matches" },
                    "done":    done,
                    "total":   total,
                    "current": query_name_for_worker,
                    "percent": percent,
                    "eta_sec": -1,
                    "errors":  0,
                }
            }));
        };

        match search::execute(&image, &scope, top_k, on_verify_progress) {
            Ok((results, failed_files)) => {
                let _ = app_clone.emit("search_complete", json!({
                    "done":         true,
                    "results":      results,
                    "failed_files": failed_files,
                }));
            }
            Err(e) => {
                let _ = app_clone.emit("search_error", json!({
                    "success": false,
                    "message": e.to_string(),
                    "data":    null
                }));
            }
        }

        search_state().lock().unwrap().running = false;
    });

    // Move the UI from "Starting…" into an active searching state. This first
    // snapshot covers query description + stage-1 ranking, which really is
    // sub-second — `on_verify_progress` above takes over with real numbers
    // once the (much slower) SIFT/RANSAC verify phase starts.
    //
    // Crucially we do NOT read the global sync/index progress here: a background
    // folder reconcile writes to that same state, and surfacing it would make the
    // search screen show phantom "Indexing 4523/10000" progress that has nothing
    // to do with the search the user just ran.
    let _ = app.emit("search_progress", json!({
        "progress": {
            "active":  true,
            "phase":   "Searching",
            "done":    0,
            "total":   0,
            "current": query_name,
            "percent": 0.0,
            "eta_sec": -1,
            "errors":  0,
        }
    }));

    // Wait for the blocking search to finish; it emits search_complete/_error.
    loop {
        time::sleep(Duration::from_millis(150)).await;
        if !search_state().lock().unwrap().running {
            break;
        }
    }
}
