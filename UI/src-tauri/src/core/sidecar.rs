//! Process lifecycle + HTTP client for the Python sidecar (Gabor/Gram
//! descriptors + SIFT/RANSAC verification — see `sidecar/` at the repo root).
//!
//! The sidecar is spawned once, hidden, and kept alive for the app's whole
//! session. Rust never touches its process directly again except to call its
//! two endpoints and, on exit, kill it. All disk writes (`meta.db`,
//! `vectors.bin`) stay owned by Rust — the sidecar only computes.

use crate::{
    config::{sidecar_bin_path, SIDECAR_HEALTH_POLL_MS, SIDECAR_PORT},
    error::{PictoriaError, Result},
    services::auth,
};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    process::Child,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};

static CHILD: Lazy<Mutex<Option<Child>>> = Lazy::new(|| Mutex::new(None));
static HEALTHY: AtomicBool = AtomicBool::new(false);
static NOTIFIED_READY: AtomicBool = AtomicBool::new(false);

fn base_url() -> String {
    format!("http://127.0.0.1:{SIDECAR_PORT}")
}

/// Shared blocking client, reused across calls rather than built fresh each
/// time. Every request sets its own `.timeout(...)` (see call sites below) —
/// without one, a stuck sidecar (one pathological file hanging the worker)
/// would leave a search or index call waiting forever with no error, which
/// is exactly what happened in practice before this existed.
static HTTP: Lazy<reqwest::blocking::Client> =
    Lazy::new(|| reqwest::blocking::Client::builder().build().expect("reqwest client"));

/// `/describe` batches can be large (a whole sync chunk); scale the timeout
/// with how many files are in the request rather than use one fixed number.
fn describe_timeout(n_paths: usize) -> std::time::Duration {
    std::time::Duration::from_secs(30 + (n_paths as u64) * 40)
}

/// `/verify` is called one candidate at a time (see `services::search`), so
/// a fixed ceiling is enough — generous relative to how long SIFT actually
/// takes (well under a second normally), to comfortably cover a search
/// waiting behind one in-flight indexing file.
const VERIFY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
const HEALTH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

// ── Lifecycle ──────────────────────────────────────────────────────────────

/// Launch the sidecar hidden and start a background thread polling `/health`.
/// In dev builds, falls back to `python server.py` if the frozen binary isn't
/// built yet, so local development doesn't require freezing on every change.
pub fn spawn() {
    let bin = sidecar_bin_path();
    let mut cmd = if bin.exists() {
        std::process::Command::new(&bin)
    } else if cfg!(debug_assertions) {
        log::warn!("[sidecar] frozen binary not found; falling back to `python server.py` (dev only)");
        let script_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("sidecar");
        let mut c = std::process::Command::new("python");
        c.arg("server.py").current_dir(script_dir);
        c
    } else {
        log::error!("[sidecar] no sidecar binary available at {:?}", bin);
        return;
    };

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    match cmd.spawn() {
        Ok(child) => {
            *CHILD.lock().unwrap() = Some(child);
            log::info!("[sidecar] process spawned, waiting for /health");
            std::thread::spawn(poll_health);
        }
        Err(e) => log::error!("[sidecar] failed to spawn: {e}"),
    }
}

/// Runs on its own thread until `/health` first answers ready, then exits —
/// this is a one-shot warm-up wait, not a continuous liveness monitor.
fn poll_health() {
    let client = reqwest::blocking::Client::new();
    loop {
        if let Ok(resp) = client.get(format!("{}/health", base_url())).send() {
            if let Ok(body) = resp.json::<HealthResp>() {
                if body.ready {
                    HEALTHY.store(true, Ordering::SeqCst);
                    log::info!("[sidecar] healthy — model loaded");
                    maybe_notify_ready();
                    return;
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(SIDECAR_HEALTH_POLL_MS));
    }
}

/// Kill the child process. Call on app exit — nothing else stops it.
pub fn shutdown() {
    if let Some(mut child) = CHILD.lock().unwrap().take() {
        let _ = child.kill();
        log::info!("[sidecar] process terminated");
    }
}

// ── Readiness ────────────────────────────────────────────────────────────

/// True once the sidecar has answered `/health` with `ready: true`.
pub fn is_healthy() -> bool {
    HEALTHY.load(Ordering::SeqCst)
}

/// The gate every indexing/search call site checks — mirrors what
/// `embedder::is_ready()` used to answer (model loaded AND a valid,
/// active session), just with "model loaded" now meaning "sidecar is up"
/// rather than "decrypted with a license-issued key".
pub fn is_ready() -> bool {
    is_healthy() && auth::has_active_session()
}

/// Call after anything that could flip `is_ready()` from false to true
/// (sidecar health achieved, or login/token-validate succeeding) — fires
/// `watcher::notify_model_ready()` exactly once per session, whichever
/// condition completes last.
pub fn maybe_notify_ready() {
    if is_ready() && !NOTIFIED_READY.swap(true, Ordering::SeqCst) {
        crate::core::watcher::notify_model_ready();
    }
}

/// Reset the one-shot notify flag on logout, so a later login can re-trigger it.
pub fn reset_notified() {
    NOTIFIED_READY.store(false, Ordering::SeqCst);
}

// ── HTTP client ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct HealthResp {
    ready: bool,
}

#[derive(Clone, Copy)]
pub enum Priority {
    Search,
    Index,
}

impl Priority {
    fn as_str(self) -> &'static str {
        match self {
            Priority::Search => "search",
            Priority::Index => "index",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Descriptor {
    pub rose: Vec<f32>,
    pub gram: Vec<Vec<f32>>,
}

#[derive(Debug, Clone, Default)]
pub struct VerifyResult {
    pub matched: bool,
    pub inliers: u32,
    pub good_matches: u32,
    pub inlier_ratio: f32,
    /// Query's four corners projected into the candidate, pixel coordinates.
    pub quad: Option<[[f32; 2]; 4]>,
    pub candidate_size: Option<(u32, u32)>,
    pub scale: Option<f32>,
}

#[derive(Serialize)]
struct DescribeReq<'a> {
    paths: &'a [String],
    priority: &'a str,
}

#[derive(Deserialize)]
struct DescribeRespItem {
    path: String,
    rose: Option<Vec<f32>>,
    gram: Option<Vec<Vec<f32>>>,
}

#[derive(Deserialize)]
struct DescribeResp {
    results: Vec<DescribeRespItem>,
}

/// Describe one or more images. Entries the sidecar couldn't decode come back
/// as `None` rather than failing the whole batch.
pub fn describe(paths: &[String], priority: Priority) -> Result<Vec<(String, Option<Descriptor>)>> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let client = reqwest::blocking::Client::new();
    let body = DescribeReq { paths, priority: priority.as_str() };
    let resp: DescribeResp = client
        .post(format!("{}/describe", base_url()))
        .json(&body)
        .send()
        .map_err(|e| PictoriaError::Fatal(format!("sidecar /describe unreachable: {e}")))?
        .json()
        .map_err(|e| PictoriaError::Fatal(format!("sidecar /describe bad response: {e}")))?;

    Ok(resp
        .results
        .into_iter()
        .map(|r| {
            let desc = match (r.rose, r.gram) {
                (Some(rose), Some(gram)) => Some(Descriptor { rose, gram }),
                _ => None,
            };
            (r.path, desc)
        })
        .collect())
}


#[derive(Serialize)]
struct VerifyReq<'a> {
    query_path: &'a str,
    candidate_paths: &'a [String],
    priority: &'a str,
}

#[derive(Deserialize)]
struct VerifyRespItem {
    path: String,
    matched: bool,
    #[serde(default)]
    inliers: u32,
    #[serde(default)]
    good_matches: u32,
    #[serde(default)]
    inlier_ratio: f32,
    #[serde(rename = "box")]
    quad: Option<[[f32; 2]; 4]>,
    candidate_size: Option<(u32, u32)>,
    scale: Option<f32>,
}

#[derive(Deserialize)]
struct VerifyResp {
    results: Vec<VerifyRespItem>,
}

/// Geometrically verify `query_path` against each of `candidate_paths`.
/// Always call with `Priority::Search` for a user-initiated search — that's
/// what lets it cut ahead of any indexing work in progress.
pub fn verify(query_path: &str, candidate_paths: &[String], priority: Priority) -> Result<Vec<(String, VerifyResult)>> {
    if candidate_paths.is_empty() {
        return Ok(Vec::new());
    }
    let client = reqwest::blocking::Client::new();
    let body = VerifyReq { query_path, candidate_paths, priority: priority.as_str() };
    let resp: VerifyResp = client
        .post(format!("{}/verify", base_url()))
        .json(&body)
        .send()
        .map_err(|e| PictoriaError::Fatal(format!("sidecar /verify unreachable: {e}")))?
        .json()
        .map_err(|e| PictoriaError::Fatal(format!("sidecar /verify bad response: {e}")))?;

    Ok(resp
        .results
        .into_iter()
        .map(|r| {
            (
                r.path,
                VerifyResult {
                    matched: r.matched,
                    inliers: r.inliers,
                    good_matches: r.good_matches,
                    inlier_ratio: r.inlier_ratio,
                    quad: r.quad,
                    candidate_size: r.candidate_size,
                    scale: r.scale,
                },
            )
        })
        .collect())
}
