use tauri::Emitter;
use tauri_plugin_updater::UpdaterExt;

/// Check GitHub releases for a newer version.
/// Emits `update_available` with { version, notes } or `update_not_available`.
#[tauri::command]
pub async fn check_for_update(app: tauri::AppHandle) {
    match app.updater() {
        Err(e) => {
            log::warn!("[updater] not available: {e}");
        }
        Ok(updater) => match updater.check().await {
            Ok(Some(update)) => {
                let _ = app.emit("update_available", serde_json::json!({
                    "version": update.version,
                    "notes":   update.body.unwrap_or_default(),
                }));
            }
            Ok(None) => {
                let _ = app.emit("update_not_available", serde_json::json!({}));
            }
            Err(e) => {
                log::warn!("[updater] check failed: {e}");
            }
        },
    }
}

/// Download and install the latest update, streaming progress back to Angular.
/// Emits `update_progress` { downloaded, total } then restarts the app.
#[tauri::command]
pub async fn install_update(app: tauri::AppHandle) {
    let updater = match app.updater() {
        Ok(u) => u,
        Err(e) => {
            let _ = app.emit("update_error", serde_json::json!({ "message": e.to_string() }));
            return;
        }
    };

    // NOTE: this is a *second* `check()` — `check_for_update` already ran one
    // to raise the banner. Every outcome has to report something, because by
    // the time this command runs the UI has already switched to its
    // downloading state. `Ok(None)` returning silently meant a bar stuck at 0%
    // with no download, no error and no timeout: indistinguishable from a slow
    // network, and unfalsifiable from the user's side.
    let update = match updater.check().await {
        Ok(Some(u)) => u,
        Ok(None) => {
            log::warn!("[updater] install requested but re-check reports no update available");
            let _ = app.emit("update_error", serde_json::json!({
                "message": "The update is no longer being offered. Restart Pictoria and try again.",
            }));
            return;
        }
        Err(e) => {
            log::warn!("[updater] re-check before install failed: {e}");
            let _ = app.emit("update_error", serde_json::json!({ "message": e.to_string() }));
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

                let _ = app_clone.emit("update_progress", serde_json::json!({
                    "downloaded": downloaded,
                    "total":      total,
                }));
            },
            || {},
        )
        .await;

    match result {
        Ok(_)  => app.restart(),
        Err(e) => {
            let _ = app.emit("update_error", serde_json::json!({ "message": e.to_string() }));
        }
    }
}
