use crate::{
    config::{OFFLINE_GRACE_SECS, SUPABASE_EDGE, TOKEN_FILE},
    core::sidecar,
    services::license,
};
use keyring::Entry;
use once_cell::sync::Lazy;
use serde_json::Value;
use std::{
    fs,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

// ── In-memory session state ───────────────────────────────────────────────
//
// Only the JWT token is persisted to disk (~/.pictoria_token).
// Everything else (user_id, user profile, model key) lives here for the
// lifetime of the process.  On logout the token file is deleted and this
// struct is cleared.

struct Session {
    user_id: String,
}

static SESSION: Lazy<Mutex<Option<Session>>> = Lazy::new(|| Mutex::new(None));

/// Separate from `SESSION` on purpose: an expired/exhausted subscription still
/// leaves the user logged in (they need the session to see billing/renewal
/// screens) but must block search/indexing — see `has_active_session()`.
static SUBSCRIPTION_OK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

// In-memory cache of the auth token.
//
// The token lives in the OS keychain, but reading it on macOS pops the
// "<app> wants to use your confidential information stored in your keychain"
// prompt whenever the running binary isn't in the item's ACL — which is the
// case on every unsigned `cargo tauri dev` rebuild, since the code signature
// (and therefore the ACL entry) changes each build. `saved_token()` is called
// very frequently (route guards, periodic re-validation, every search, the
// profile screen), so without this cache the prompt reappears many times per
// session.
//
// Caching the token means the keychain is touched at most ONCE per process
// launch; every later check is served from memory. The cache is kept in sync
// by `save_token()` (login) and `delete_token()` (logout / invalid session),
// so it never goes stale. Only a present token is cached — a `None` result is
// never cached, so a cold start with a saved token still reads the store once
// instead of wrongly reporting "logged out".
static TOKEN_CACHE: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));

fn set_session(user_id: &str, _user: &Value) {
    *SESSION.lock().unwrap() = Some(Session { user_id: user_id.to_string() });
}

fn clear_in_memory_session() {
    *SESSION.lock().unwrap() = None;
}

/// User ID from the current in-memory session (None if not logged in).
pub fn session_user_id() -> Option<String> {
    SESSION.lock().unwrap().as_ref().map(|s| s.user_id.clone())
}

/// True while a user is logged in AND their subscription is current. This is
/// the gate `core::sidecar::is_ready()` checks — it's what used to be
/// enforced by needing a license-issued key to decrypt the model.
pub fn has_active_session() -> bool {
    session_user_id().is_some() && SUBSCRIPTION_OK.load(std::sync::atomic::Ordering::SeqCst)
}

// ── Token storage ─────────────────────────────────────────────────────────
//
// Preferred: the OS credential store (macOS Keychain / Windows Credential
// Manager) via the `keyring` crate — the token never sits in a plaintext
// dotfile readable by any other process running as the same user.
//
// Fallback: the original `~/.pictoria_token` plaintext file. Used on platforms
// without a native keychain backend compiled in (where `keyring` falls back
// to a non-persistent in-memory "mock" store that would silently lose the
// token on every restart) and if a keychain call fails for any reason
// (locked keychain, headless environment, etc.).
//
// Existing installs that already have `~/.pictoria_token` are migrated into
// the keychain transparently the first time `saved_token()` runs.

const KEYRING_SERVICE: &str = "com.pictoria.app";
const KEYRING_USER:    &str = "auth_token";

/// Whether this platform has a real, persistent keyring backend compiled in
/// (kept in sync with the `keyring` feature flags in Cargo.toml). On any
/// other OS the crate transparently falls back to its in-memory "mock"
/// store, which would appear to work but lose the token on every restart —
/// so on those platforms we skip the keychain entirely and use the file.
fn keyring_available() -> bool {
    cfg!(any(target_os = "macos", target_os = "windows"))
}

fn keyring_entry() -> Option<Entry> {
    if !keyring_available() {
        return None;
    }
    Entry::new(KEYRING_SERVICE, KEYRING_USER).ok()
}

fn save_token(token: &str) {
    *TOKEN_CACHE.lock().unwrap() = Some(token.to_string());
    if let Some(entry) = keyring_entry() {
        if entry.set_password(token).is_ok() {
            // Stored securely — remove any legacy plaintext copy.
            let _ = fs::remove_file(TOKEN_FILE.as_path());
            return;
        }
    }
    let _ = fs::write(TOKEN_FILE.as_path(), token);
}

fn delete_token() {
    *TOKEN_CACHE.lock().unwrap() = None;
    if let Some(entry) = keyring_entry() {
        let _ = entry.delete_credential();
    }
    let _ = fs::remove_file(TOKEN_FILE.as_path());
}

pub fn saved_token() -> Option<String> {
    // Fast path: serve the token from memory so the keychain (and its access
    // prompt) is never hit more than once per process launch.
    if let Some(token) = TOKEN_CACHE.lock().unwrap().clone() {
        return Some(token);
    }

    let token = read_token_from_store();
    if let Some(token) = &token {
        *TOKEN_CACHE.lock().unwrap() = Some(token.clone());
    }
    token
}

/// Read the token from the OS keychain (or legacy file fallback). This is the
/// only place that actually touches the credential store — keep it behind the
/// `TOKEN_CACHE` so it runs at most once per launch.
fn read_token_from_store() -> Option<String> {
    if let Some(entry) = keyring_entry() {
        if let Ok(token) = entry.get_password() {
            let token = token.trim().to_string();
            if !token.is_empty() {
                return Some(token);
            }
        }
    }

    // Not in the keychain (or no keychain on this platform) — fall back to
    // the legacy file, migrating it into the keychain if possible so this
    // branch isn't needed again next time.
    let file_token = fs::read_to_string(TOKEN_FILE.as_path())
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    if let (Some(token), Some(entry)) = (&file_token, keyring_entry()) {
        if entry.set_password(token).is_ok() {
            let _ = fs::remove_file(TOKEN_FILE.as_path());
        }
    }

    file_token
}

// ── Public API ────────────────────────────────────────────────────────────

pub async fn login(email: &str, password: &str) -> Value {
    let device_id = license::device_id();

    let result = reqwest::Client::new()
        .post(format!("{SUPABASE_EDGE}/login-user-test"))
        .json(&serde_json::json!({
            "email":     email,
            "password":  password,
            "device_id": device_id,
        }))
        .send()
        .await;

    match result {
        Err(e) => network_error(e),
        Ok(resp) => {
            let data: Value = match resp.json().await {
                Ok(v)  => v,
                Err(_) => return server_error(),
            };

            if !data["success"].as_bool().unwrap_or(false) {
                return data;
            }

            let token   = data["token"].as_str().unwrap_or_default();
            let user_id = data["user"]["id"].as_str().unwrap_or_default();

            save_token(token);
            set_session(user_id, &data["user"]);
            sidecar::maybe_notify_ready(); // sidecar may already be healthy and waiting on this login

            serde_json::json!({
                "success": true,
                "message": data["message"].as_str().unwrap_or("Login successful"),
                "data": {
                    "token": token,
                    "user":  data["user"],
                }
            })
        }
    }
}

/// Called on app startup: validates the saved token, confirms the
/// subscription is current, and stores user info in memory.
pub async fn validate_saved_token() -> Value {
    let token = match saved_token() {
        Some(t) => t,
        None    => return no_session(),
    };

    let device_id = license::device_id();

    let result = reqwest::Client::new()
        .get(format!("{SUPABASE_EDGE}/validate-token-test"))
        .header("Authorization", format!("Bearer {token}"))
        .header("x-device-id",   &device_id)
        .send()
        .await;

    match result {
        Err(e) => network_error(e),
        Ok(resp) => {
            let data: Value = match resp.json().await {
                Ok(v)  => v,
                Err(_) => return server_error(),
            };

            if !data["valid"].as_bool().unwrap_or(false) {
                // Session lost — a fresh login is required.
                delete_token();
                clear_in_memory_session();
                sidecar::reset_notified();
                return serde_json::json!({
                    "success": false,
                    "message": data["message"].as_str().unwrap_or("Session expired. Please login again."),
                    "data":    null
                });
            }

            // Populate in-memory session — nothing written to disk.
            if let Some(uid) = data["user"]["id"].as_str() {
                set_session(uid, &data["user"]);
            }
            mark_valid_now();

            // If subscription has lapsed, block search/indexing even if the UI is
            // bypassed — the user stays logged in (they still need to see
            // billing/renewal screens), but has_active_session() now reports
            // false, so sidecar::is_ready() does too.
            let status = data["user"]["subscription_status"].as_str().unwrap_or("");
            let subscription_ok = status != "expired" && status != "exhausted";
            SUBSCRIPTION_OK.store(subscription_ok, std::sync::atomic::Ordering::SeqCst);
            if subscription_ok {
                // Sidecar may already be healthy and waiting on this; also
                // registers the OS watchers + runs one reconciliation pass the
                // first time this session that both conditions are true.
                sidecar::maybe_notify_ready();
            } else {
                sidecar::reset_notified();
            }

            serde_json::json!({
                "success": true,
                "message": "Session valid",
                "data":    { "user": data["user"] }
            })
        }
    }
}

pub fn logout() -> Value {
    delete_token();
    clear_in_memory_session();
    sidecar::reset_notified();
    serde_json::json!({
        "success": true,
        "message": "Logged out successfully",
        "data":    null
    })
}

// ── Periodic re-validation ───────────────────────────────────────────────
//
// `validate_saved_token()` runs at cold start and before search, but a long
// -running session (tray app, left open for days) would otherwise never
// re-check its subscription status against Supabase. The UI calls
// `periodic_revalidate()` on a timer (see master.ts) to close that gap.
//
// To keep the app usable offline for short periods (flights, spotty wifi)
// without caching an "expired but still works" session indefinitely, a
// network failure here is tolerated for up to `OFFLINE_GRACE_SECS` since the
// last successful validation — after that, the session is torn down and the
// model unloaded just like an explicit "invalid" response.

/// Unix timestamp (seconds) of the last successful `validate-token-test`
/// round-trip this session. `None` until the first success.
static LAST_VALID_AT: Lazy<Mutex<Option<i64>>> = Lazy::new(|| Mutex::new(None));

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn mark_valid_now() {
    *LAST_VALID_AT.lock().unwrap() = Some(now_secs());
}

/// Background re-validation tick. Returns `{"success": true, "data": {"action": ..., "user"?: ...}}`
/// where `action` is one of:
///   - `"none"`       — not logged in, nothing to do.
///   - `"ok"`         — re-validated successfully; `user` carries the latest profile.
///   - `"ok-offline"` — Supabase unreachable, but still inside the offline
///                       grace window — keep the current session as-is.
///   - `"logout"`     — session is invalid/expired, or the offline grace
///                       period has been exceeded; the caller (UI) must log
///                       the user out.
pub async fn periodic_revalidate() -> Value {
    if saved_token().is_none() {
        return serde_json::json!({ "success": true, "message": "No session", "data": { "action": "none" } });
    }

    let result = validate_saved_token().await;

    if result["success"].as_bool().unwrap_or(false) {
        return serde_json::json!({
            "success": true,
            "message": "Session re-validated",
            "data": { "action": "ok", "user": result["data"]["user"] }
        });
    }

    // `validate_saved_token()` already deletes the token and tears down the
    // session/model when Supabase explicitly says the token is invalid. If
    // that happened, there's nothing left to grace-period — log out now.
    if saved_token().is_none() {
        return serde_json::json!({ "success": true, "message": "Session expired", "data": { "action": "logout" } });
    }

    // Otherwise this was a network-level failure (no internet, DNS, etc.)
    // with the token still intact. Allow a short offline grace period before
    // forcing a logout.
    let last_valid = LAST_VALID_AT.lock().unwrap().unwrap_or_else(now_secs);
    if now_secs() - last_valid < OFFLINE_GRACE_SECS {
        return serde_json::json!({ "success": true, "message": "Offline - using cached session", "data": { "action": "ok-offline" } });
    }

    delete_token();
    clear_in_memory_session();
    sidecar::reset_notified();
    serde_json::json!({ "success": true, "message": "Offline grace period exceeded", "data": { "action": "logout" } })
}

// ── Search count ─────────────────────────────────────────────────────────

/// Tell the backend a search actually completed, so `users.search_count` can
/// be incremented (see the `record-search` edge function). A dedicated
/// endpoint rather than piggybacking on `validate-token-test` — that one also
/// runs at login and on `periodic_revalidate`'s timer, so incrementing there
/// would count logins and idle re-checks as searches, not just searches.
///
/// Best-effort and fire-and-forget by design (see the call site in
/// `commands::search::start_search`): a user's search results are already
/// computed and on screen by the time this runs, and a failed count-increment
/// is not something the user should ever see or have their search blocked by.
pub async fn record_search() {
    let Some(token) = saved_token() else { return };

    let result = reqwest::Client::new()
        .post(format!("{SUPABASE_EDGE}/record-search"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await;

    match result {
        Ok(resp) if resp.status().is_success() => log::debug!("[auth] search recorded"),
        Ok(resp) => log::warn!("[auth] record-search returned {}", resp.status()),
        Err(e) => log::warn!("[auth] record-search unreachable: {e}"),
    }
}

/// Request a one-time verification code be emailed to `email` (registration).
pub async fn send_otp(email: &str) -> Value {
    let result = reqwest::Client::new()
        .post(format!("{SUPABASE_EDGE}/send-otp"))
        .json(&serde_json::json!({ "email": email }))
        .send()
        .await;

    match result {
        Err(e) => network_error(e),
        Ok(resp) => resp.json().await.unwrap_or_else(|_| server_error()),
    }
}

pub async fn register_request(
    first_name:   &str,
    last_name:    &str,
    email:        &str,
    password:     &str,
    phone_number: Option<&str>,
    company_name: Option<&str>,
    otp_code:     &str,
) -> Value {
    let result = reqwest::Client::new()
        .post(format!("{SUPABASE_EDGE}/register-request"))
        .json(&serde_json::json!({
            "first_name":   first_name,
            "last_name":    last_name,
            "email":        email,
            "password":     password,
            "phone_number": phone_number,
            "company_name": company_name,
            "otp_code":     otp_code,
            "device_id":    license::device_id(),
        }))
        .send()
        .await;

    match result {
        Err(e) => network_error(e),
        Ok(resp) => resp.json().await.unwrap_or_else(|_| server_error()),
    }
}

// ── Forgot password ──────────────────────────────────────────────────────
//
// Two-step flow, mirroring the registration OTP flow above:
//   1. `forgot_password_send_otp`   — emails a 6-digit code to a *registered*
//      address.
//   2. `forgot_password_verify_otp` — verifies that code, then Supabase
//      generates a new random password, sets it on the account, and emails
//      it to the user. Nothing is written to disk here — the user logs in
//      normally afterwards with the new password.

/// Step 1 — request a one-time code be emailed to `email` for password reset.
pub async fn forgot_password_send_otp(email: &str) -> Value {
    let result = reqwest::Client::new()
        .post(format!("{SUPABASE_EDGE}/forgot-password-send-otp"))
        .json(&serde_json::json!({ "email": email }))
        .send()
        .await;

    match result {
        Err(e) => network_error(e),
        Ok(resp) => resp.json().await.unwrap_or_else(|_| server_error()),
    }
}

/// Step 2 — verify the code; on success Supabase resets the account's
/// password to a freshly-generated random value and emails it to the user.
pub async fn forgot_password_verify_otp(email: &str, otp_code: &str) -> Value {
    let result = reqwest::Client::new()
        .post(format!("{SUPABASE_EDGE}/forgot-password-verify-otp"))
        .json(&serde_json::json!({ "email": email, "otp_code": otp_code }))
        .send()
        .await;

    match result {
        Err(e) => network_error(e),
        Ok(resp) => resp.json().await.unwrap_or_else(|_| server_error()),
    }
}

// ── Change password ───────────────────────────────────────────────────────
//
// Authenticated, in-app password change from the profile screen. The user
// supplies their current password (verified server-side) and a new one. The
// saved JWT identifies the account — Supabase never trusts a client-supplied
// user id for this. On success the caller (UI) logs the user out so they must
// sign in again with the new credentials.

/// Change the logged-in user's password. Sends the saved session token so the
/// edge function can identify the account and verify `old_password` before
/// applying `new_password`.
pub async fn change_password(old_password: &str, new_password: &str) -> Value {
    let token = match saved_token() {
        Some(t) => t,
        None    => return no_session(),
    };

    let result = reqwest::Client::new()
        .post(format!("{SUPABASE_EDGE}/change-password"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({
            "old_password": old_password,
            "new_password": new_password,
        }))
        .send()
        .await;

    match result {
        Err(e) => network_error(e),
        Ok(resp) => resp.json().await.unwrap_or_else(|_| server_error()),
    }
}

// ── Error helpers ─────────────────────────────────────────────────────────

fn no_session() -> Value {
    serde_json::json!({ "success": false, "message": "No saved session", "data": null })
}

fn server_error() -> Value {
    serde_json::json!({ "success": false, "message": "Invalid response from server", "data": null })
}

fn network_error(e: reqwest::Error) -> Value {
    let msg = if e.is_connect() || e.is_timeout() {
        "No internet connection. Please connect and try again.".to_string()
    } else {
        format!("Network error: {e}")
    };
    serde_json::json!({ "success": false, "message": msg, "data": null })
}
