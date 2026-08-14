//! Process lifecycle + HTTP client for the Python sidecar (Gabor/Gram
//! descriptors + SIFT/RANSAC verification — see `sidecar/` at the repo root).
//!
//! The sidecar is spawned once, hidden, and kept alive for the app's whole
//! session, then watched by a background thread for the rest of it — see
//! "Lifecycle" below for startup cleanup and crash recovery. Rust never
//! touches its process directly again except to call its two endpoints and,
//! on exit, kill it. All disk writes (`meta.db`, `vectors.bin`) stay owned
//! by Rust — the sidecar only computes.

use crate::{
    config::{sidecar_bin_path, SIDECAR_FILE, SIDECAR_HEALTH_POLL_MS, SIDECAR_PORT},
    error::{PictoriaError, Result},
    services::auth,
};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    path::PathBuf,
    process::Child,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::Duration,
};
use tauri::{AppHandle, Emitter};

static CHILD: Lazy<Mutex<Option<Child>>> = Lazy::new(|| Mutex::new(None));
static HEALTHY: AtomicBool = AtomicBool::new(false);
static NOTIFIED_READY: AtomicBool = AtomicBool::new(false);

fn base_url() -> String {
    format!("http://127.0.0.1:{SIDECAR_PORT}")
}

/// Shared blocking client, reused across calls rather than built fresh each
/// time. Every request sets its own `.timeout(...)` (see call sites below) —
/// without one, a stuck sidecar (one pathological file hanging the worker)
/// would leave a search or index call waiting forever with no error.
///
/// This client existed unused for a while (along with `describe_timeout`,
/// the old `VERIFY_TIMEOUT`, and `HEALTH_TIMEOUT` below) — every call site
/// was building its own untimed `reqwest::blocking::Client::new()` instead,
/// which is how `/verify` batches went unbounded: with no client-side
/// ceiling, a 200-candidate shortlist that took 70-90s serially (see
/// `sidecar/pipeline.py`'s `verify_one`) just hung until *something*
/// upstream — never identified, possibly OS/AV-related — cut the connection
/// at a suspiciously consistent ~30s, and `search::execute`'s
/// `.unwrap_or_default()` silently turned that into "nothing verified".
static HTTP: Lazy<reqwest::blocking::Client> =
    Lazy::new(|| reqwest::blocking::Client::builder().build().expect("reqwest client"));

/// `/describe` batches can be large (a whole sync chunk); scale the timeout
/// with how many files are in the request rather than use one fixed number.
fn describe_timeout(n_paths: usize) -> std::time::Duration {
    std::time::Duration::from_secs(30 + (n_paths as u64) * 40)
}

/// `/verify` runs the whole shortlist as one batch, parallelized server-side
/// across an 8-way thread pool (`sidecar/server.py`'s `_run`) — real-world
/// cost is ~50-60ms/candidate at that parallelism, well under this budget's
/// 1s/candidate; the generous margin is for a cold start (PyInstaller
/// self-extraction + AV scan) or a shortlist arriving behind an in-flight
/// indexing file rather than steady-state compute.
fn verify_timeout(n_candidates: usize) -> std::time::Duration {
    std::time::Duration::from_secs(30 + n_candidates as u64)
}

const HEALTH_TIMEOUT: Duration = Duration::from_secs(3);

/// How often the watchdog checks whether the child process is still alive,
/// once it's past the initial boot wait.
const WATCHDOG_POLL_MS: u64 = 2000;
/// Pause before relaunching after a crash — avoids a tight respawn loop
/// hammering the CPU/log if the sidecar is failing to start at all.
const CRASH_RESPAWN_DELAY_MS: u64 = 1500;
/// Stop auto-relaunching after this many crashes with no successful
/// recovery in between, and surface a terminal error instead — a sidecar
/// that dies immediately every time needs a human, not an infinite retry loop.
const MAX_CONSECUTIVE_CRASHES: u32 = 5;

// ── Lifecycle ──────────────────────────────────────────────────────────────
//
// 1. Startup: `kill_orphaned_sidecars` sweeps up anything left running from a
//    previous session that didn't exit cleanly (crash, force-quit, power
//    loss) before spawning a fresh process — otherwise the new process fails
//    to bind `SIDECAR_PORT` and Rust ends up silently talking to the stale
//    orphan instead of the one it thinks it just launched.
// 2. Crash recovery: `watchdog` runs for the whole app session. It waits for
//    the fresh process to become healthy, then watches it; if the process
//    exits on its own, that's a crash — `watchdog` marks it unhealthy,
//    emits `sidecar_crashed` so the UI can say so, and relaunches it.
// 3. Shutdown: unchanged — `shutdown()` kills the child, called from every
//    app-exit path (see `lib.rs`). It also doubles as `watchdog`'s signal to
//    stop watching: once `CHILD` is `None`, a live process "exiting" is
//    expected, not a crash.

/// Kill any already-running sidecar process before spawning a fresh one —
/// best-effort; a failed kill just means the spawn below fails to bind the
/// port, which the watchdog's health wait will keep retrying against anyway.
fn kill_orphaned_sidecars() {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        match std::process::Command::new("taskkill")
            .args(["/F", "/IM", SIDECAR_FILE])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
        {
            Ok(out) if out.status.success() => log::info!("[sidecar] killed an orphaned {SIDECAR_FILE} from a previous session"),
            _ => {} // nothing was running — the common case, not worth logging
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        match std::process::Command::new("pkill").args(["-f", SIDECAR_FILE]).output() {
            Ok(out) if out.status.success() => log::info!("[sidecar] killed an orphaned {SIDECAR_FILE} from a previous session"),
            _ => {}
        }
    }
}

/// Kill any orphan, launch the sidecar hidden, then hand off to `watchdog`
/// for the rest of the app's session (initial health wait + crash recovery).
pub fn spawn(app: AppHandle) {
    kill_orphaned_sidecars();
    spawn_process();
    std::thread::spawn(move || watchdog(app));
}

/// Just the OS-level launch — building the command, hiding the console
/// window, storing the `Child`. Split out from `spawn()` so `watchdog` can
/// call this again on a crash without re-running the orphan sweep (the
/// process it would be "sweeping" is the one that just died, not an orphan).
fn spawn_process() {
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
        }
        Err(e) => log::error!("[sidecar] failed to spawn: {e}"),
    }
}

/// What became of the child process since the watchdog last checked.
enum ChildState {
    /// Still running.
    Alive,
    /// Exited on its own (or `try_wait` itself errored) while `CHILD` still
    /// held it — i.e. nobody asked it to stop. A crash.
    Exited,
    /// `CHILD` is `None` — `shutdown()` already took and killed it. Not a
    /// crash; the watchdog's cue to stop watching.
    ShutDown,
}

fn child_state() -> ChildState {
    let mut guard = CHILD.lock().unwrap();
    match guard.as_mut() {
        None => ChildState::ShutDown,
        Some(child) => match child.try_wait() {
            Ok(None) => ChildState::Alive,
            Ok(Some(_)) | Err(_) => ChildState::Exited,
        },
    }
}

/// Runs for the whole app session on its own thread: waits for `/health` to
/// report ready (initial boot, or after a respawn below), then watches the
/// child process until it exits. A clean `shutdown()` ends the thread; an
/// unexpected exit is treated as a crash — `HEALTHY` drops, `sidecar_crashed`
/// tells the UI, and the process is relaunched after a short delay.
fn watchdog(app: AppHandle) {
    let mut consecutive_crashes: u32;

    loop {
        wait_for_health();
        consecutive_crashes = 0; // reaching healthy again clears the streak

        loop {
            std::thread::sleep(Duration::from_millis(WATCHDOG_POLL_MS));
            match child_state() {
                ChildState::Alive => continue,
                ChildState::ShutDown => return,
                ChildState::Exited => break,
            }
        }

        HEALTHY.store(false, Ordering::SeqCst);
        consecutive_crashes += 1;
        log::warn!("[sidecar] process exited unexpectedly (attempt {consecutive_crashes}) — relaunching");

        if consecutive_crashes > MAX_CONSECUTIVE_CRASHES {
            log::error!("[sidecar] gave up after {consecutive_crashes} consecutive crashes");
            let _ = app.emit("sidecar_crashed", json!({
                "recovering": false,
                "message": "The AI engine keeps failing to start. Try restarting Pictoria.",
            }));
            return;
        }

        let _ = app.emit("sidecar_crashed", json!({
            "recovering": true,
            "message": "The AI engine stopped unexpectedly — restarting it now…",
        }));

        std::thread::sleep(Duration::from_millis(CRASH_RESPAWN_DELAY_MS));
        spawn_process();
    }
}

/// Poll `/health` until it reports ready. Used both for the initial boot
/// wait and every post-crash respawn. Deliberately doesn't emit anything on
/// success — that would fire `sidecar_crashed` on every normal first boot
/// too. The frontend clears its own "reconnecting" state by noticing
/// `sidecar_status` go ready again (see `search.ts`'s re-armed poll), which
/// only happens once it's actually been told a crash occurred.
fn wait_for_health() {
    let client = reqwest::blocking::Client::new();
    loop {
        if let Ok(resp) = client.get(format!("{}/health", base_url())).timeout(HEALTH_TIMEOUT).send() {
            if let Ok(body) = resp.json::<HealthResp>() {
                if body.ready {
                    HEALTHY.store(true, Ordering::SeqCst);
                    log::info!("[sidecar] healthy — model loaded");
                    maybe_notify_ready();
                    return;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(SIDECAR_HEALTH_POLL_MS));
    }
}

/// Kill the child process. Call on app exit — nothing else stops it. Also
/// what tells `watchdog` a process exit was intentional, not a crash (see
/// `ChildState::ShutDown`).
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
    /// COLOR_DIM-length colour histogram — computed sidecar-side now (see
    /// `pipeline.color_histogram`) from the same decode used for rose/gram,
    /// rather than a separate full-resolution decode in Rust.
    pub color: Vec<f32>,
    /// 1-2 dominant colour bucket names for the Browse colour tag, from the
    /// same decode (`pipeline.dominant_colors`). Empty when the sidecar
    /// predates this field — callers treat that as "no tag", never an error.
    pub dominant: Vec<String>,
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
    color: Option<Vec<f32>>,
    /// Absent from an older sidecar build — defaulted rather than required, so
    /// a version skew costs the colour *tag* only, not the whole descriptor.
    #[serde(default)]
    dominant: Vec<String>,
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
    let body = DescribeReq { paths, priority: priority.as_str() };
    let resp: DescribeResp = HTTP
        .post(format!("{}/describe", base_url()))
        .timeout(describe_timeout(paths.len()))
        .json(&body)
        .send()
        .map_err(|e| PictoriaError::Fatal(format!("sidecar /describe unreachable: {e}")))?
        .json()
        .map_err(|e| PictoriaError::Fatal(format!("sidecar /describe bad response: {e}")))?;

    Ok(resp
        .results
        .into_iter()
        .map(|r| {
            let desc = match (r.rose, r.gram, r.color) {
                (Some(rose), Some(gram), Some(color)) => {
                    Some(Descriptor { rose, gram, color, dominant: r.dominant })
                }
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
    let body = VerifyReq { query_path, candidate_paths, priority: priority.as_str() };
    let resp: VerifyResp = HTTP
        .post(format!("{}/verify", base_url()))
        .timeout(verify_timeout(candidate_paths.len()))
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
