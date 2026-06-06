//! Tauri command handlers for the search pipeline.
//!
//! The model is loaded lazily — only on the first search.  Angular's doSearch()
//! calls validate-token, receives the onnx_key in the response, and forwards it
//! here.  The key is used to decrypt and load the ONNX session if it is not
//! already in memory, then immediately goes out of scope and is dropped.
//!
//! Progress is streamed to Angular via three Tauri events:
//!   search_progress  — snapshot while running
//!   search_complete  — final results
//!   search_error     — fatal error that stopped the search

use crate::{
    core::embedder,
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

/// `scope_paths` empty or omitted → search every watched folder in the Library.
/// Otherwise the search is restricted to the provided folders.
#[tauri::command]
pub async fn start_search(
    app:         tauri::AppHandle,
    image_path:  String,
    scope_paths: Option<Vec<String>>,
    top_k:       usize,
    onnx_key:    Option<String>,
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

    // Load the model now if it is not already in memory.
    if !embedder::is_ready() {
        match onnx_key {
            Some(ref key) => {
                if let Err(e) = embedder::load_model(key) {
                    let _ = app.emit("search_error", json!({
                        "success": false,
                        "message": format!("Failed to load AI model: {e}"),
                        "data":    null
                    }));
                    search_state().lock().unwrap().running = false;
                    return;
                }
            }
            None => {
                let _ = app.emit("search_error", json!({
                    "success": false,
                    "message": "AI model not ready. Please try searching again.",
                    "data":    null
                }));
                search_state().lock().unwrap().running = false;
                return;
            }
        }
    }
    // onnx_key goes out of scope here — dropped from memory.

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
