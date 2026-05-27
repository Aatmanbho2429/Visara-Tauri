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

use clipboard_rs::{Clipboard, ClipboardContext, common::RustImage};
use commands::{auth, hotkey, search, subscription, update};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

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

// ── Global hot-key handler ────────────────────────────────────────────────

/// Ctrl+Shift+V on Windows/Linux, ⌘+Shift+V on macOS.
fn hotkey_shortcut() -> Shortcut {
    #[cfg(target_os = "macos")]
    let mods = Modifiers::SUPER | Modifiers::SHIFT;
    #[cfg(not(target_os = "macos"))]
    let mods = Modifiers::CONTROL | Modifiers::SHIFT;

    Shortcut::new(Some(mods), Code::KeyV)
}

/// Recognized image file extensions for file-list clipboard contents.
const IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "png", "tif", "tiff", "psd", "psb", "bmp", "gif", "webp"];

fn is_image_file(p: &PathBuf) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| IMAGE_EXTS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Read the clipboard and resolve it to an image file path.
///
/// 1. Bitmap (screenshots, "Copy Image" from browser, Photoshop, etc.)
///    → written to `<temp>/visara_clipboard.png` (overwritten each call)
/// 2. File list (Windows Explorer "Copy", macOS Finder "Copy")
///    → first image file in the list is returned directly, no temp copy
fn resolve_clipboard_image() -> Option<PathBuf> {
    let ctx = match ClipboardContext::new() {
        Ok(c) => c,
        Err(e) => {
            log::warn!("[hotkey] could not open clipboard: {e}");
            return None;
        }
    };

    // ── 1. Bitmap clipboard ───────────────────────────────────────────
    match ctx.get_image() {
        Ok(img) => {
            let (w, h) = img.get_size();
            let dest   = hotkey::temp_path();
            match img.save_to_path(dest.to_string_lossy().as_ref()) {
                Ok(_) => {
                    log::info!("[hotkey] saved {w}x{h} clipboard bitmap → {:?}", dest);
                    return Some(dest);
                }
                Err(e) => log::warn!("[hotkey] failed to write clipboard bitmap: {e}"),
            }
        }
        Err(e) => log::info!("[hotkey] no bitmap in clipboard ({e}); trying file list"),
    }

    // ── 2. File-list clipboard ────────────────────────────────────────
    match ctx.get_files() {
        Ok(files) => {
            for f in &files {
                let path = PathBuf::from(f.trim_start_matches("file://"));
                if is_image_file(&path) && path.exists() {
                    log::info!("[hotkey] using clipboard file directly → {:?}", path);
                    return Some(path);
                }
            }
            log::info!("[hotkey] clipboard file list had no recognized image ({} files)", files.len());
        }
        Err(e) => log::info!("[hotkey] no file list in clipboard ({e})"),
    }

    None
}

/// Fired when the user presses the global hot-key from anywhere on the system.
fn on_global_hotkey(app: &AppHandle) {
    let image_path = resolve_clipboard_image()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();

    // Bring the main window forward regardless — user pressed the hot-key for a reason.
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }

    let _ = app.emit("hotkey_pressed", serde_json::json!({
        "has_image":  !image_path.is_empty(),
        "image_path": image_path,
    }));
}

// ── Application entry point ────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Resolve bundled-resource directory once and stash it in config.
            // This is the only reliable way to locate packaged resources in
            // Tauri v2 — there is no `TAURI_RESOURCE_DIR` env var at runtime.
            match app.path().resource_dir() {
                Ok(res_dir) => {
                    log::info!("[startup] resource_dir = {:?}", res_dir);
                    config::set_resource_dir(res_dir);
                }
                Err(e) => log::warn!(
                    "[startup] could not resolve resource_dir ({e}); \
                     falling back to dev path"
                ),
            }

            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Debug)
                        .build(),
                )?;
            }

            // Register the global hot-key.  Silent failure is acceptable —
            // the user just won't see the hot-key behave (another app may
            // already own the combo).
            if let Err(e) = app.global_shortcut().register(hotkey_shortcut()) {
                log::warn!("[hotkey] failed to register Ctrl+Shift+V: {e}");
            } else {
                log::info!("[hotkey] registered Ctrl+Shift+V");
            }

            Ok(())
        })
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state == ShortcutState::Pressed && *shortcut == hotkey_shortcut() {
                        on_global_hotkey(app);
                    }
                })
                .build(),
        )
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
