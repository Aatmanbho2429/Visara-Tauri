//! Tauri command handlers for the Browse grid.

use crate::{core::thumbs, services::browse};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

#[tauri::command]
pub fn browse_directory(app: AppHandle, path: String) {
    let result = browse::browse(path);
    let _ = app.emit("browse_directory_response", result);
}

/// Generate (or fetch the cached) thumbnail for one image and return its path.
///
/// Unlike the other commands this returns the value **directly** instead of
/// emitting an event: the grid requests dozens of thumbnails concurrently, and a
/// shared `<command>_response` event would cross-wire those calls.  Tauri v2
/// resolves the returned value straight to the JS `invoke()` promise.
#[tauri::command]
pub async fn get_thumbnail(path: String) -> Result<String, String> {
    let joined = tauri::async_runtime::spawn_blocking(move || {
        thumbs::ensure_thumb(&PathBuf::from(&path))
    })
    .await
    .map_err(|e| e.to_string())?;

    joined
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| e.to_string())
}
