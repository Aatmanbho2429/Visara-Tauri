use crate::{
    config::{SUPABASE_EDGE, TOKEN_FILE},
    core::{embedder, watcher},
    services::license,
};
use once_cell::sync::Lazy;
use serde_json::Value;
use std::{fs, sync::Mutex};

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

// ── Token file helpers ────────────────────────────────────────────────────

fn save_token(token: &str) {
    let _ = fs::write(TOKEN_FILE.as_path(), token);
}

fn delete_token() {
    let _ = fs::remove_file(TOKEN_FILE.as_path());
}

pub fn saved_token() -> Option<String> {
    fs::read_to_string(TOKEN_FILE.as_path())
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
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

pub async fn register_request(
    first_name:   &str,
    last_name:    &str,
    email:        &str,
    password:     &str,
    phone_number: Option<&str>,
    company_name: Option<&str>,
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
