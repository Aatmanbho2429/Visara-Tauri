use crate::models::response::{ApiResponse, ResponseUpdateAvailable, ResponseUpdateProgress};
use tauri::Emitter;
use tauri_plugin_updater::UpdaterExt;

fn emit_update_error(app: &tauri::AppHandle, message: impl Into<String>) {
    let payload: ApiResponse<()> = ApiResponse::err(500, message);
    let _ = app.emit("update_error", payload);
}

// Check GitHub releases for a newer version.
// Emits `update_available` with { version, notes } or `update_not_available`.
#[tauri::command]
pub async fn update_check(app: tauri::AppHandle) {
    match app.updater() {
        Err(e) => {
            log::warn!("[updater] not available: {e}");
        }
        Ok(updater) => match updater.check().await {
            Ok(Some(update)) => {
                let payload = ApiResponse::ok(ResponseUpdateAvailable {
                    version: update.version,
                    notes:   update.body.unwrap_or_default(),
                });
                let _ = app.emit("update_available", payload);
            }
            Ok(None) => {
                let payload: ApiResponse<()> = ApiResponse::ok_empty("No update available");
                let _ = app.emit("update_not_available", payload);
            }
            Err(e) => {
                log::warn!("[updater] check failed: {e}");
            }
        },
    }
}

// Download and install the latest update, streaming progress back to Angular.
// Emits `update_progress` { downloaded, total } then restarts the app.
#[tauri::command]
pub async fn update_install(app: tauri::AppHandle) {
    let updater = match app.updater() {
        Ok(u) => u,
        Err(e) => {
            emit_update_error(&app, e.to_string());
            return;
        }
    };

    // NOTE: this is a *second* `check()` — `update_check` already ran one
    // to raise the banner. Every outcome has to report something, because by
    // the time this command runs the UI has already switched to its
    // downloading state. `Ok(None)` returning silently meant a bar stuck at 0%
    // with no download, no error and no timeout: indistinguishable from a slow
    // network, and unfalsifiable from the user's side.
    let update = match updater.check().await {
        Ok(Some(u)) => u,
        Ok(None) => {
            log::warn!("[updater] install requested but re-check reports no update available");
            emit_update_error(&app, "The update is no longer being offered. Restart Pictoria and try again.");
            return;
        }
        Err(e) => {
            log::warn!("[updater] re-check before install failed: {e}");
            emit_update_error(&app, e.to_string());
            return;
        }
    };

    log::info!("[updater] starting download of v{}", update.version);

    let app_clone = app.clone();

    // `download_and_install` calls this per chunk with `chunk.len()` — the size
    // of *that chunk*, not a running total (see `on_chunk(chunk.len(), ...)` in
    // tauri-plugin-updater's `Update::download`). Emitting it straight through
    // as "downloaded" is why the progress bar never moved: the UI computes
    // downloaded/total, and a ~16KB chunk against a ~200MB installer is 0%
    // every single time. Accumulate here so the field means what its name says.
    let mut downloaded: u64 = 0;
    // Only emit when the whole-number percentage actually changes. A 200MB
    // download is tens of thousands of chunks, and one IPC message each would
    // flood the webview with redundant events for a bar that can only render
    // 100 distinct states.
    let mut last_pct: Option<u64> = None;

    let result = update
        .download_and_install(
            move |chunk_len, total| {
                downloaded = downloaded.saturating_add(chunk_len as u64);

                let pct = total
                    .filter(|t| *t > 0)
                    .map(|t| (downloaded.saturating_mul(100) / t).min(100));
                // With no Content-Length there is no percentage to change, so
                // fall back to emitting every chunk and let the UI show an
                // indeterminate state off the null total.
                if pct.is_some() && pct == last_pct {
                    return;
                }

                // Logged so a "the bar isn't moving" report can be split in
                // half without guesswork: if these lines appear, the backend
                // is streaming fine and the problem is in the UI; if they
                // don't, the download itself never started reporting.
                match (pct, last_pct) {
                    (Some(p), _) if p % 10 == 0 => {
                        log::info!("[updater] download {p}% ({downloaded} / {total:?} bytes)")
                    }
                    (None, None) => {
                        log::info!("[updater] download progressing, no Content-Length \
                                    (UI shows an indeterminate bar)")
                    }
                    _ => {}
                }

                last_pct = pct;

                let payload = ApiResponse::ok(ResponseUpdateProgress { downloaded, total });
                let _ = app_clone.emit("update_progress", payload);
            },
            || {},
        )
        .await;

    match result {
        Ok(_)  => app.restart(),
        Err(e) => emit_update_error(&app, e.to_string()),
    }
}

// docs/plans/force-update.md §2.3 — manual-download fallback for when the
// in-app installer itself is broken (bad signature, missing platform
// artifact, `latest.json` malformed). The URL is a constant, never accepted
// from the webview — never give the frontend an "open any URL" primitive.
// Follows the same per-platform shell-out shape as `file_commands::file_open_path`.
#[tauri::command]
pub fn update_open_releases_page(app: tauri::AppHandle, request_id: Option<String>) {
    const RELEASES_URL: &str = "https://github.com/Aatmanbho2429/Visara-Tauri/releases/latest";

    #[cfg(target_os = "windows")]
    let spawn_result = std::process::Command::new("cmd")
        .args(["/C", "start", "", RELEASES_URL])
        .spawn();

    #[cfg(target_os = "macos")]
    let spawn_result = std::process::Command::new("open").arg(RELEASES_URL).spawn();

    #[cfg(target_os = "linux")]
    let spawn_result = std::process::Command::new("xdg-open").arg(RELEASES_URL).spawn();

    let result: ApiResponse<()> = match spawn_result {
        Ok(_) => ApiResponse::ok_empty("Opened"),
        Err(e) => {
            log::warn!("[updater] could not open releases page in browser: {e}");
            ApiResponse::err(500, e.to_string())
        }
    };
    let _ = app.emit("update_open_releases_page_response", result.with_request_id(request_id));
}

// §2.4 — the window's X hides Pictoria to the tray rather than quitting, so
// the force-update overlay's Quit button needs a real exit path. Mirrors the
// tray's own `tray_quit` handler in `lib.rs` exactly. No response event: the
// process is gone by the time one could be observed.
#[tauri::command]
pub fn update_quit_app(app: tauri::AppHandle) {
    crate::core::sidecar::shutdown();
    app.exit(0);
}
