//! Pictoria — Tauri application root.
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
use commands::{auth, browse, catalog, hotkey, library, search, subscription, tags, update};
use std::path::PathBuf;
use tauri::{
    AppHandle, Emitter, Manager, WindowEvent,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tauri_plugin_notification::NotificationExt;

/// CLI flag passed by the OS when the app is launched at user login.
/// Detected in `setup()` to keep the main window hidden (tray-only boot).
const AUTOSTART_FLAG: &str = "--autostart";

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
/// 1. File list (macOS Finder "Copy", Windows Explorer "Copy")
///    → first image file in the list is returned directly, no temp copy
/// 2. Bitmap (screenshots, "Copy Image" from browser, Photoshop, etc.)
///    → written to `<temp>/pictoria_clipboard.png` (overwritten each call)
///
/// The file list is checked FIRST and on purpose.  When the user copies an
/// actual image file we always want the real, full-resolution file on disk.
/// On macOS a Finder file-copy ALSO exposes an `NSImage` representation that is
/// merely the file's icon/thumbnail, so reading the bitmap first (as we used to)
/// embedded a tiny generic preview and returned unrelated matches.  Windows
/// Explorer copies expose no bitmap at all — which is why this only misbehaved
/// on macOS.  The bitmap branch now only runs for genuine bitmap clipboards
/// (screenshots, browser "Copy Image"), which carry no file path.
fn resolve_clipboard_image() -> Option<PathBuf> {
    let ctx = match ClipboardContext::new() {
        Ok(c) => c,
        Err(e) => {
            log::warn!("[hotkey] could not open clipboard: {e}");
            return None;
        }
    };

    // ── 1. File-list clipboard ────────────────────────────────────────
    match ctx.get_files() {
        Ok(files) => {
            for f in &files {
                let path = PathBuf::from(f.trim_start_matches("file://"));
                if is_image_file(&path) && path.exists() {
                    log::info!("[hotkey] using clipboard file directly → {:?}", path);
                    return Some(path);
                }
            }
            log::info!("[hotkey] clipboard file list had no recognized image ({} files); trying bitmap", files.len());
        }
        Err(e) => log::info!("[hotkey] no file list in clipboard ({e}); trying bitmap"),
    }

    // ── 2. Bitmap clipboard ───────────────────────────────────────────
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
        Err(e) => log::info!("[hotkey] no bitmap in clipboard ({e})"),
    }

    None
}

/// Fired when the user presses the global hot-key from anywhere on the system.
fn on_global_hotkey(app: &AppHandle) {
    let image_path = resolve_clipboard_image()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();

    // Bring the main window forward regardless — user pressed the hot-key for a reason.
    show_main_window(app);

    let _ = app.emit("hotkey_pressed", serde_json::json!({
        "has_image":  !image_path.is_empty(),
        "image_path": image_path,
    }));
}

// ── System tray ────────────────────────────────────────────────────────────

/// Reveal + focus the main window. Safe to call whether the window is hidden,
/// minimized, or already visible behind other apps.
fn show_main_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

/// Build the persistent tray icon with menu: Open, Check Updates, Quit.
/// Silent failure (logged warning) — tray is a UX enhancement, not critical.
fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let show_item   = MenuItem::with_id(app, "tray_show",   "Open Pictoria",         true, None::<&str>)?;
    let update_item = MenuItem::with_id(app, "tray_update", "Check for Updates",   true, None::<&str>)?;
    let quit_item   = MenuItem::with_id(app, "tray_quit",   "Quit Pictoria",         true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;

    let menu = Menu::with_items(app, &[&show_item, &sep1, &update_item, &sep2, &quit_item])?;

    let icon = match app.default_window_icon() {
        Some(i) => i.clone(),
        None => {
            log::warn!("[tray] no default window icon available; skipping tray setup");
            return Ok(());
        }
    };

    TrayIconBuilder::with_id("main-tray")
        .icon(icon)
        .tooltip("Pictoria — AI Image Search")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "tray_show" => show_main_window(app),
            "tray_update" => {
                let app_clone = app.clone();
                tauri::async_runtime::spawn(async move {
                    update::check_for_update(app_clone).await;
                });
            }
            "tray_quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

/// Intercept the X button on the main window: hide to tray instead of quitting.
/// First close per session triggers a native OS notification so the user
/// understands the app is still running.  Other windows close normally.
fn on_window_event(window: &tauri::Window, event: &WindowEvent) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static NOTIFIED_THIS_SESSION: AtomicBool = AtomicBool::new(false);

    if let WindowEvent::CloseRequested { api, .. } = event {
        if window.label() == "main" {
            api.prevent_close();
            let _ = window.hide();

            if !NOTIFIED_THIS_SESSION.swap(true, Ordering::SeqCst) {
                let body = if cfg!(target_os = "macos") {
                    "Click the Pictoria icon in the menu bar, or press ⌘+Shift+V, to bring it back."
                } else {
                    "Click the Pictoria icon in the system tray, or press Ctrl+Shift+V, to bring it back."
                };
                let _ = window.app_handle()
                    .notification()
                    .builder()
                    .title("Pictoria is still running")
                    .body(body)
                    .show();
            }
        }
    }
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

            // Build the persistent system tray.  Failure is non-fatal — the
            // app remains usable, just without the tray icon.
            if let Err(e) = setup_tray(app.handle()) {
                log::warn!("[tray] failed to build tray: {e}");
            }

            // Start the background folder watcher.  Its OS subscriptions and
            // initial reconciliation kick in later (after token validation
            // loads the CLIP model) via `crate::core::watcher::refresh_active_watches`
            // and `reconcile_all`.
            crate::core::watcher::init(app.handle().clone());

            // Spawn the NAS auto-recovery loop (macOS only — no-op elsewhere).
            // Checks every 30 s for missing folders with a stored network URL
            // and remounts them silently using Keychain credentials.
            crate::core::watcher::start_nas_recovery_loop();

            // Run one-time migrations before anything reads the DB or index:
            //  • normalise file_tags to reference files(id) with ON DELETE CASCADE
            //  • if the embedding schema changed, arm a one-time re-index (the
            //    watcher's post-login reconcile rebuilds every folder's vectors).
            crate::core::migrate::run_startup();

            // Backfill network_url for folders added before this feature shipped.
            // Runs in the background so startup is not blocked.
            crate::core::watcher::backfill_network_urls();

            // One-time sweep of tag rows orphaned by earlier folder deletes /
            // renames, so the Browse facets don't keep showing files that are
            // no longer indexed.  Cheap and idempotent; safe to run every boot.
            if let Ok(con) = crate::core::database::open() {
                match crate::core::database::sweep_orphan_tags(&con) {
                    Ok(n) if n > 0 => log::info!("[db] swept {n} orphaned tag row(s)"),
                    Ok(_)          => {}
                    Err(e)         => log::warn!("[db] orphan tag sweep failed: {e}"),
                }
            }

            // When launched at user login via autostart, the OS passes
            // `--autostart` on the command line.  In that case we keep the
            // main window hidden so Pictoria boots silently into the tray.
            let launched_via_autostart = std::env::args().any(|a| a == AUTOSTART_FLAG);
            if launched_via_autostart {
                log::info!("[autostart] launched via autostart — starting hidden in tray");
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.hide();
                }
            }

            Ok(())
        })
        .on_window_event(on_window_event)
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![AUTOSTART_FLAG]),
        ))
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
            auth::auth_periodic_revalidate,
            auth::auth_check_session,
            auth::auth_logout,
            auth::auth_send_otp,
            auth::auth_forgot_password_send_otp,
            auth::auth_forgot_password_verify_otp,
            auth::auth_change_password,
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
            // ── Library / watched folders ────────────────────────────
            library::library_list_folders,
            library::library_add_folder,
            library::library_remove_folder,
            library::library_set_paused,
            library::library_rescan_folder,
            library::library_stats,
            library::library_folder_tree,
            // ── Tags ─────────────────────────────────────────────────
            tags::tags_set,
            tags::tags_remove,
            tags::tags_get,
            tags::tags_facets,
            tags::tags_query,
            tags::tags_suggest,
            tags::tags_backfill_colors,
            // ── Browse ───────────────────────────────────────────────
            browse::browse_directory,
            browse::get_thumbnail,
            browse::get_catalog_image,
            // ── Catalog themes ───────────────────────────────────────
            catalog::catalog_save_theme,
            catalog::catalog_list_themes,
            catalog::catalog_get_theme,
            catalog::catalog_delete_theme,
            catalog::catalog_save_pdf,
            // ── Utilities ────────────────────────────────────────────
            open_file_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Pictoria");
}
