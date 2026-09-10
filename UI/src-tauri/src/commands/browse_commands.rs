// Tauri command handlers for the Browse grid.

use crate::{core::thumbs, models::response::{ApiResponse, ResponseGetThumbnail}, services::browse};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

#[tauri::command]
pub fn browse_directory(app: AppHandle, path: String, request_id: Option<String>) {
    let result = browse::browse(path).with_request_id(request_id);
    let _ = app.emit("browse_directory_response", result);
}

// Generate (or fetch the cached) thumbnail for one image.
//
// The grid requests dozens of thumbnails concurrently, which is exactly the
// case `request_id` correlation exists for: ZoneWrapperService matches each
// `browse_get_thumbnail_response` event back to the specific call that asked
// for it, so one shared event stream can't cross-wire two in-flight requests.
#[tauri::command]
pub async fn browse_get_thumbnail(app: AppHandle, path: String, request_id: Option<String>) {
    let joined = tauri::async_runtime::spawn_blocking(move || {
        thumbs::ensure_thumb(&PathBuf::from(&path))
    })
    .await;

    let result: ApiResponse<ResponseGetThumbnail> = match joined {
        Ok(Ok(p)) => ApiResponse::ok(ResponseGetThumbnail { path: p.to_string_lossy().to_string() }),
        Ok(Err(e)) => ApiResponse::err(500, e.to_string()),
        Err(e) => ApiResponse::err(500, e.to_string()),
    };
    let _ = app.emit("browse_get_thumbnail_response", result.with_request_id(request_id));
}
