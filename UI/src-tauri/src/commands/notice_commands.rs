// Tauri command handlers for the one-time post-reset notice.
//
// When a release ships a full library wipe (`core::migrate::run_library_reset`)
// the user loses their watched-folder setup, so the UI owes them an
// explanation. These two commands are how the banner finds out it should show
// and how it records that it has been dismissed. Both now follow the same
// emit-`<command>_response` convention as every other command, per the
// project's envelope rules — there's no more direct-return special case.

use crate::{core::migrate, models::response::ApiResponse};
use tauri::Emitter;

// True while the post-reset banner still needs showing.
#[tauri::command]
pub fn notice_reset_pending(app: tauri::AppHandle, request_id: Option<String>) {
    let pending = migrate::reset_notice_pending();
    let result = ApiResponse::ok(pending).with_request_id(request_id);
    let _ = app.emit("notice_reset_pending_response", result);
}

// Called when the user closes the banner. Backed by a file, so the dismissal
// survives a restart — and so does the notice if they quit without dismissing.
#[tauri::command]
pub fn notice_dismiss_reset(app: tauri::AppHandle, request_id: Option<String>) {
    migrate::clear_reset_notice();
    let result: ApiResponse<()> = ApiResponse::ok_empty("Dismissed").with_request_id(request_id);
    let _ = app.emit("notice_dismiss_reset_response", result);
}
