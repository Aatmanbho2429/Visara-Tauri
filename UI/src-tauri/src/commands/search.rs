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
    config::VECTOR_STORE_PATH,
    core::{embedder, progress, vector_store::VectorStore},
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

#[tauri::command]
pub async fn start_search(
    app:         tauri::AppHandle,
    image_path:  String,
    folder_path: String,
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
    // The onnx_key came from Angular's validate-token call moments ago;
    // it is a local variable and will be dropped when this block ends.
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

    let app_clone = app.clone();

    // Spawn the heavy work on a blocking thread so Tokio stays responsive.
    tokio::task::spawn_blocking(move || {
        let image  = PathBuf::from(&image_path);
        let folder = PathBuf::from(&folder_path);

        match search::execute(&image, &folder, top_k) {
            Ok((results, failed_files)) => {
                if let Ok(store) = VectorStore::load(VECTOR_STORE_PATH.as_path()) {
                    let _ = store.save(VECTOR_STORE_PATH.as_path());
                }

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

    // Stream progress snapshots every 500 ms until the blocking task finishes.
    loop {
        time::sleep(Duration::from_millis(500)).await;

        let still_running = search_state().lock().unwrap().running;

        let snap = progress::get_progress();
        if snap.active || still_running {
            let _ = app.emit("search_progress", json!({ "progress": snap }));
        }

        if !still_running { break; }
    }
}
