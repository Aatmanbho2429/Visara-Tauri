//! Tauri command handlers for the one-time post-reset notice.
//!
//! When a release ships a full library wipe (`core::migrate::run_library_reset`)
//! the user loses their watched-folder setup, so the UI owes them an
//! explanation. These two commands are how the banner finds out it should show
//! and how it records that it has been dismissed.
//!
//! Both return their value directly rather than emitting a `<command>_response`
//! event like the rest of `commands::` — there is no long-running work here and
//! nothing to stream, so the request/response round-trip `invoke()` already
//! gives us is enough.

use crate::core::migrate;

/// True while the post-reset banner still needs showing.
#[tauri::command]
pub fn reset_notice_pending() -> bool {
    migrate::reset_notice_pending()
}

/// Called when the user closes the banner. Backed by a file, so the dismissal
/// survives a restart — and so does the notice if they quit without dismissing.
#[tauri::command]
pub fn dismiss_reset_notice() {
    migrate::clear_reset_notice();
}
