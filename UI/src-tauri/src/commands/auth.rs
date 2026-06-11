//! Tauri command handlers for authentication.
//!
//! Each handler calls the corresponding service function and emits a Tauri
//! event with the JSON result.  Angular's `TauriService.invoke()` listens for
//! these events by convention: `{command_name}_response`.

use crate::services::auth;
use tauri::Emitter;

#[tauri::command]
pub async fn auth_login(app: tauri::AppHandle, email: String, password: String) {
    let result = auth::login(&email, &password).await;
    let _ = app.emit("auth_login_response", result);
}

#[tauri::command]
pub async fn auth_validate_token(app: tauri::AppHandle) {
    let result = auth::validate_saved_token().await;
    let _ = app.emit("auth_validate_token_response", result);
}

/// Periodic background re-check of the session/subscription against
/// Supabase, called on a timer from the UI (see master.ts). See
/// `services::auth::periodic_revalidate` for the possible `action` values.
#[tauri::command]
pub async fn auth_periodic_revalidate(app: tauri::AppHandle) {
    let result = auth::periodic_revalidate().await;
    let _ = app.emit("auth_periodic_revalidate_response", result);
}

/// Instant check — no network call.  Returns success:true if a token file
/// exists on disk, false if the user has never logged in or has logged out.
/// Used by route guards to avoid a Supabase round-trip on every navigation.
#[tauri::command]
pub fn auth_check_session(app: tauri::AppHandle) {
    let exists = auth::saved_token().is_some();
    let _ = app.emit("auth_check_session_response", serde_json::json!({
        "success": exists,
        "message": if exists { "Session exists" } else { "No session" },
        "data":    null
    }));
}

#[tauri::command]
pub fn auth_logout(app: tauri::AppHandle) {
    let result = auth::logout();
    let _ = app.emit("auth_logout_response", result);
}

/// Email a one-time verification code for registration.
#[tauri::command]
pub async fn auth_send_otp(app: tauri::AppHandle, email: String) {
    let result = auth::send_otp(&email).await;
    let _ = app.emit("auth_send_otp_response", result);
}

#[tauri::command]
pub async fn auth_request_access(
    app:          tauri::AppHandle,
    first_name:   String,
    last_name:    String,
    email:        String,
    password:     String,
    phone_number: Option<String>,
    company_name: Option<String>,
    otp_code:     String,
) {
    let result = auth::register_request(
        &first_name,
        &last_name,
        &email,
        &password,
        phone_number.as_deref(),
        company_name.as_deref(),
        &otp_code,
    )
    .await;
    let _ = app.emit("auth_request_access_response", result);
}
