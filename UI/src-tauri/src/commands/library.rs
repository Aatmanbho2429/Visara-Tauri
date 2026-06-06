//! Tauri command handlers for watched-folder library management.
//!
//! Each handler delegates to `services::library` and emits the standard
//! `<command>_response` event that Angular's `TauriService.invoke()` listens
//! for.  Long-running work (initial scan, re-scan) is spawned off the
//! invoke thread by the service layer.

use crate::services::library;
use tauri::Emitter;

#[tauri::command]
pub fn library_list_folders(app: tauri::AppHandle) {
    let result = library::list_folders();
    let _ = app.emit("library_list_folders_response", result);
}

#[tauri::command]
pub fn library_add_folder(app: tauri::AppHandle, path: String) {
    let result = library::add_folder(path);
    let _ = app.emit("library_add_folder_response", result);
}

#[tauri::command]
pub fn library_remove_folder(app: tauri::AppHandle, path: String, purge: bool) {
    let result = library::remove_folder(path, purge);
    let _ = app.emit("library_remove_folder_response", result);
}

#[tauri::command]
pub fn library_set_paused(app: tauri::AppHandle, path: String, paused: bool) {
    let result = library::set_paused(path, paused);
    let _ = app.emit("library_set_paused_response", result);
}

#[tauri::command]
pub fn library_rescan_folder(app: tauri::AppHandle, path: String) {
    let result = library::rescan_folder(path);
    let _ = app.emit("library_rescan_folder_response", result);
}

#[tauri::command]
pub fn library_stats(app: tauri::AppHandle) {
    let result = library::stats();
    let _ = app.emit("library_stats_response", result);
}

#[tauri::command]
pub fn library_folder_tree(app: tauri::AppHandle, path: String) {
    let result = library::folder_tree(path);
    let _ = app.emit("library_folder_tree_response", result);
}
