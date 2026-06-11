use crate::{
    config::{OFFLINE_GRACE_SECS, SUPABASE_EDGE, TOKEN_FILE},
    core::{embedder, watcher},
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
// Only the JWT token is persisted to disk (~/.visara_token).
// Everything else (user_id, user profile, model key) lives here for the
// lifetime of the process.  On logout the token file is deleted and this
// struct is cleared.

struct Session {
    user_id: String,
}

static SESSION: Lazy<Mutex<Option<Session>>> = Lazy::new(|| Mutex::new(None));

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

// ── Token storage ─────────────────────────────────────────────────────────
//
// Preferred: the OS credential store (macOS Keychain / Windows Credential
// Manager) via the `keyring` crate — the token never sits in a plaintext
// dotfile readable by any other process running as the same user.
//
// Fallback: the original `~/.visara_token` plaintext file. Used on platforms
// without a native keychain backend compiled in (where `keyring` falls back
// to a non-persistent in-memory "mock" store that would silently lose the
// token on every restart) and if a keychain call fails for any reason
// (locked keychain, headless environment, etc.).
//
// Existing installs that already have `~/.visara_token` are migrated into
// the keychain transparently the first time `saved_token()` runs.

const KEYRING_SERVICE: &str = "com.visara.app";
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
    if let Some(entry) = keyring_entry() {
        let _ = entry.delete_credential();
    }
    let _ = fs::remove_file(TOKEN_FILE.as_path());
}

pub fn saved_token() -> Option<String> {
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

            // Model is NOT loaded here — it loads on the first search via
            // the onnx_key the client receives from validate_saved_token().
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

/// Called on app startup: validates the saved token, loads the model from the
/// onnx_key Supabase returns, and stores user info in memory.
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
                // Session lost — unload the model so a fresh login is required
                // to get the onnx_key again.  Nothing usable left in memory.
                delete_token();
                clear_in_memory_session();
                embedder::reset();
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

            // If subscription has lapsed, unload the model so it cannot be used
            // even if the UI is bypassed.  Supabase also withholds onnx_key for
            // expired users so it cannot be reloaded without renewing.
            let status = data["user"]["subscription_status"].as_str().unwrap_or("");
            if status == "expired" || status == "exhausted" {
                embedder::reset();
            }

            let onnx_key = data["onnx_key"].as_str().unwrap_or("").to_string();

            // Eagerly preload the CLIP model in the background so the user's
            // first search doesn't pay the multi-second decrypt-and-compile
            // cost.  No-op if the model is already loaded (e.g. token was
            // re-validated mid-session).  The onnx_key is still returned to
            // Angular as a fallback for the rare case where the user clicks
            // Search before this background task finishes.
            if !onnx_key.is_empty() {
                let key = onnx_key.clone();
                let model_already_ready = embedder::is_ready();
                tokio::task::spawn_blocking(move || {
                    // validate_saved_token() is hit frequently — the cold-start
                    // route guard, every search, and every profile visit all call
                    // it.  The OS watchers + reconciliation only need to run when
                    // the model TRANSITIONS from not-ready to ready (cold start, or
                    // after a renewal that followed an expiry reset).  Doing it on
                    // every call re-scanned all folders needlessly and bled that
                    // "Indexing…" progress into the search screen, since search and
                    // sync shared one global progress state.  When the model is
                    // already loaded the watchers are live and catching changes, so
                    // there is nothing to redo.
                    if !model_already_ready {
                        if let Err(e) = embedder::load_model(&key) {
                            log::warn!("[auth] background model preload failed: {e}");
                            return;
                        }
                        log::info!("[auth] CLIP model preloaded after validate");

                        // First time the model is ready this session: register the
                        // OS file-system watchers and run one reconciliation pass so
                        // changes made while the app was closed are picked up (also
                        // drains folders deferred to the watcher's PENDING set).
                        watcher::notify_model_ready();
                    }
                });
            }

            serde_json::json!({
                "success": true,
                "message": "Session valid",
                "data":    { "user": data["user"], "onnx_key": onnx_key }
            })
        }
    }
}

pub fn logout() -> Value {
    delete_token();
    clear_in_memory_session();
    embedder::reset();
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
    embedder::reset();
    serde_json::json!({ "success": true, "message": "Offline grace period exceeded", "data": { "action": "logout" } })
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
