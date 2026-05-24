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

    let update = match updater.check().await {
        Ok(Some(u)) => u,
        Ok(None) => return,
        Err(e) => {
            let _ = app.emit("update_error", serde_json::json!({ "message": e.to_string() }));
            return;
        }
    };

    let app_clone = app.clone();
    let result = update
        .download_and_install(
            move |downloaded, total| {
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
