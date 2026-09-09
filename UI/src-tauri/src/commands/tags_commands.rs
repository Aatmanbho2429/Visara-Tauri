// Tauri command handlers for tagging.  All emit the `<command>_response` event
// carrying the ApiResponse envelope.

use crate::{
    core::database,
    models::{request::TagFilterDto, response::{ApiResponse, ResponseTagsUpdated}},
    services::tags,
};
use tauri::{AppHandle, Emitter};

#[tauri::command]
pub fn tags_set(app: AppHandle, paths: Vec<String>, category: String, value: String, request_id: Option<String>) {
    let result = tags::set_tags(paths, category, value).with_request_id(request_id);
    let _ = app.emit("tags_set_response", result);
}

#[tauri::command]
pub fn tags_remove(app: AppHandle, paths: Vec<String>, category: String, value: String, request_id: Option<String>) {
    let result = tags::remove_tag(paths, category, value).with_request_id(request_id);
    let _ = app.emit("tags_remove_response", result);
}

#[tauri::command]
pub fn tags_get(app: AppHandle, paths: Vec<String>, request_id: Option<String>) {
    let result = tags::get_tags(paths).with_request_id(request_id);
    let _ = app.emit("tags_get_response", result);
}

#[tauri::command]
pub fn tags_facets(app: AppHandle, request_id: Option<String>) {
    let result = tags::facets().with_request_id(request_id);
    let _ = app.emit("tags_facets_response", result);
}

#[tauri::command]
pub fn tags_query(app: AppHandle, filters: Vec<TagFilterDto>, request_id: Option<String>) {
    let pairs: Vec<(String, String)> = filters.into_iter().map(|f| (f.category, f.value)).collect();
    let result = tags::query(pairs).with_request_id(request_id);
    let _ = app.emit("tags_query_response", result);
}

#[tauri::command]
pub fn tags_suggest(app: AppHandle, paths: Vec<String>, request_id: Option<String>) {
    let result = tags::suggest(paths).with_request_id(request_id);
    let _ = app.emit("tags_suggest_response", result);
}

// Kick off auto-colour analysis for every watched folder in the background.
// Emits `tags_updated` when finished so the UI can refresh its facets.
#[tauri::command]
pub fn tags_backfill_colors(app: AppHandle, request_id: Option<String>) {
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
        let updated: ApiResponse<ResponseTagsUpdated> = ApiResponse::ok(ResponseTagsUpdated { colored: total });
        let _ = app2.emit("tags_updated", updated);
    });

    let result: ApiResponse<()> = ApiResponse::ok_empty("Colour analysis started.").with_request_id(request_id);
    let _ = app.emit("tags_backfill_colors_response", result);
}
