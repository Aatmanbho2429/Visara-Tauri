//! Visara — Tauri application root.
//!
//! All business logic lives in the modules below.  This file only wires
//! command handlers into the Tauri builder and registers platform plugins.
//! The old HTTP-proxy pattern (Rust → Python FastAPI) is gone; every command
//! now calls Rust service functions directly.

mod commands;
mod config;
mod core;
mod error;
mod services;
mod utils;

use commands::{auth, search, subscription, update};

// ── Platform file opener ───────────────────────────────────────────────────

#[tauri::command]
fn open_file_path(path: String) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("explorer")
        .args(["/select,", &path])
        .spawn();

    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open")
        .args(["-R", &path])
        .spawn();

    #[cfg(target_os = "linux")]
    {
        let folder = std::path::Path::new(&path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or(path);
        let _ = std::process::Command::new("xdg-open").arg(&folder).spawn();
    }
}

// ── Application entry point ────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Debug)
                        .build(),
                )?;
            }
            Ok(())
        })
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            // ── Authentication ───────────────────────────────────────
            auth::auth_login,
            auth::auth_validate_token,
            auth::auth_check_session,
            auth::auth_logout,
            auth::auth_request_access,
            // ── Image search ─────────────────────────────────────────
            search::start_search,
            // ── Subscription / payments ──────────────────────────────
            subscription::get_plans,
            subscription::get_user_subscriptions,
            subscription::create_order,
            subscription::verify_payment,
            // ── Updates ──────────────────────────────────────────────
            update::check_for_update,
            update::install_update,
            // ── Utilities ────────────────────────────────────────────
            open_file_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Visara");
}
