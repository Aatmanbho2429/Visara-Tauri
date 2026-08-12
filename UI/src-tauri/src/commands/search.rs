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

    // Spawn the heavy work on a blocking thread so Tokio stays responsive.
    tokio::task::spawn_blocking(move || {
        let image = PathBuf::from(&image_path);
        let scope: Vec<PathBuf> = scope_paths
            .unwrap_or_default()
            .into_iter()
            .map(PathBuf::from)
            .collect();

        match search::execute(&image, &scope, top_k) {
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

    // Move the UI from "Starting…" into an active searching state.  A search is
    // a single-image embed + cosine scan (sub-second over an indexed store), so
    // we emit one indeterminate "Searching" snapshot rather than a granular bar.
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
