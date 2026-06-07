//! Tauri command handlers for catalog themes.

use crate::services::catalog;
use tauri::{AppHandle, Emitter};

#[tauri::command]
pub fn catalog_save_theme(app: AppHandle, id: String, name: String, json: String) {
    let result = catalog::save_theme(id, name, json);
    let _ = app.emit("catalog_save_theme_response", result);
}

#[tauri::command]
pub fn catalog_list_themes(app: AppHandle) {
    let result = catalog::list_themes();
    let _ = app.emit("catalog_list_themes_response", result);
}

#[tauri::command]
pub fn catalog_get_theme(app: AppHandle, id: String) {
    let result = catalog::get_theme(id);
    let _ = app.emit("catalog_get_theme_response", result);
}

#[tauri::command]
pub fn catalog_delete_theme(app: AppHandle, id: String) {
    let result = catalog::delete_theme(id);
    let _ = app.emit("catalog_delete_theme_response", result);
}

#[tauri::command]
pub fn catalog_save_pdf(app: AppHandle, path: String, base64: String) {
    let result = catalog::save_pdf(path, base64);
    let _ = app.emit("catalog_save_pdf_response", result);
}
