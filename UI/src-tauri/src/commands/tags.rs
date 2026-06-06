//! Tauri command handlers for tagging.  All emit the `<command>_response` event
//! that Angular's `TauriService.invoke()` listens for.

use crate::{core::database, services::tags};
use tauri::{AppHandle, Emitter};

#[derive(serde::Deserialize)]
pub struct TagFilter {
    pub category: String,
    pub value:    String,
}

#[tauri::command]
pub fn tags_set(app: AppHandle, paths: Vec<String>, category: String, value: String) {
    let result = tags::set_tags(paths, category, value);
    let _ = app.emit("tags_set_response", result);
}

#[tauri::command]
pub fn tags_remove(app: AppHandle, paths: Vec<String>, category: String, value: String) {
    let result = tags::remove_tag(paths, category, value);
    let _ = app.emit("tags_remove_response", result);
}

#[tauri::command]
pub fn tags_get(app: AppHandle, paths: Vec<String>) {
    let result = tags::get_tags(paths);
    let _ = app.emit("tags_get_response", result);
}

#[tauri::command]
pub fn tags_facets(app: AppHandle) {
    let result = tags::facets();
    let _ = app.emit("tags_facets_response", result);
}

#[tauri::command]
pub fn tags_query(app: AppHandle, filters: Vec<TagFilter>) {
    let pairs: Vec<(String, String)> = filters.into_iter().map(|f| (f.category, f.value)).collect();
    let result = tags::query(pairs);
    let _ = app.emit("tags_query_response", result);
}

#[tauri::command]
pub fn tags_suggest(app: AppHandle, paths: Vec<String>) {
    let result = tags::suggest(paths);
    let _ = app.emit("tags_suggest_response", result);
}

/// Kick off auto-colour analysis for every watched folder in the background.
/// Emits `tags_updated` when finished so the UI can refresh its facets.
#[tauri::command]
pub fn tags_backfill_colors(app: AppHandle) {
    let app2 = app.clone();
    std::thread::spawn(move || {
        let roots: Vec<String> = database::open()
            .and_then(|c| database::list_watched_folders(&c))
            .map(|v| v.into_iter().map(|f| f.path).collect())
            .unwrap_or_default();

        let mut total = 0usize;
        for r in &roots {
            total += tags::backfill_colors(r);
        }
        let _ = app2.emit("tags_updated", serde_json::json!({ "colored": total }));
    });

    let _ = app.emit("tags_backfill_colors_response", serde_json::json!({
        "success": true,
        "message": "Colour analysis started.",
        "data":    null,
    }));
}
