// Tauri command handlers for authentication.
//
// Each handler calls the corresponding service function and emits a Tauri
// event carrying the ApiResponse envelope.  Angular's ZoneWrapperService.invoke()
// listens for these events by convention: `{command_name}_response`, matching
// on `requestId` so a concurrent duplicate call can't steal another caller's
// response.

use crate::{models::response::ApiResponse, services::auth};
use tauri::Emitter;

#[tauri::command]
pub async fn auth_login(app: tauri::AppHandle, email: String, password: String, request_id: Option<String>) {
    let result = auth::login(&email, &password).await.with_request_id(request_id);
    let _ = app.emit("auth_login_response", result);
}

#[tauri::command]
pub async fn auth_validate_token(app: tauri::AppHandle, request_id: Option<String>) {
    let result = auth::validate_saved_token().await.with_request_id(request_id);
    let _ = app.emit("auth_validate_token_response", result);
}

// Periodic background re-check of the session/subscription against
// Supabase, called on a timer from the UI (see master.ts). See
// `services::auth::periodic_revalidate` for the possible `action` values.
#[tauri::command]
pub async fn auth_periodic_revalidate(app: tauri::AppHandle, request_id: Option<String>) {
    let result = auth::periodic_revalidate().await.with_request_id(request_id);
    let _ = app.emit("auth_periodic_revalidate_response", result);
}

// Instant check — no network call.  200 if a token file exists on disk, 401
// if the user has never logged in or has logged out.  Used by route guards
// to avoid a Supabase round-trip on every navigation.
#[tauri::command]
pub fn auth_check_session(app: tauri::AppHandle, request_id: Option<String>) {
    let result: ApiResponse<()> = auth::check_session().with_request_id(request_id);
    let _ = app.emit("auth_check_session_response", result);
}

#[tauri::command]
pub fn auth_logout(app: tauri::AppHandle, request_id: Option<String>) {
    let result = auth::logout().with_request_id(request_id);
    let _ = app.emit("auth_logout_response", result);
}

// Email a one-time verification code for registration.
#[tauri::command]
pub async fn auth_send_otp(app: tauri::AppHandle, email: String, request_id: Option<String>) {
    let result = auth::send_otp(&email).await.with_request_id(request_id);
    let _ = app.emit("auth_send_otp_response", result);
}

// Forgot-password step 1 — email a one-time verification code to a
// registered address.
#[tauri::command]
pub async fn auth_forgot_password_send_otp(app: tauri::AppHandle, email: String, request_id: Option<String>) {
    let result = auth::forgot_password_send_otp(&email).await.with_request_id(request_id);
    let _ = app.emit("auth_forgot_password_send_otp_response", result);
}

// Forgot-password step 2 — verify the code; on success Supabase resets the
// account password and emails the new one to the user.
#[tauri::command]
pub async fn auth_forgot_password_verify_otp(app: tauri::AppHandle, email: String, otp_code: String, request_id: Option<String>) {
    let result = auth::forgot_password_verify_otp(&email, &otp_code).await.with_request_id(request_id);
    let _ = app.emit("auth_forgot_password_verify_otp_response", result);
}

// Change the logged-in user's password. The saved session token identifies
// the account; the edge function verifies `old_password` before applying
// `new_password`. On success the UI logs the user out.
#[tauri::command]
pub async fn auth_change_password(app: tauri::AppHandle, old_password: String, new_password: String, request_id: Option<String>) {
    let result = auth::change_password(&old_password, &new_password).await.with_request_id(request_id);
    let _ = app.emit("auth_change_password_response", result);
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
    request_id:   Option<String>,
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
    .await
    .with_request_id(request_id);
    let _ = app.emit("auth_request_access_response", result);
}
