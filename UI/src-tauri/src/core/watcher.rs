//! Cross-platform background file-system watcher.
//!
//! Subscribes to OS-level change notifications (`ReadDirectoryChangesW` on
//! Windows, `FSEvents` on macOS, `inotify` on Linux) for every registered
//! watched folder.  When events arrive, they are coalesced into per-folder
//! resync requests with a debounce window so 50 quick paste-creates trigger
//! a single sync, not 50.
//!
//! The actual embedding work delegates to `services::sync::sync_folder`,
//! which already handles hash dedupe, mtime caching, tombstoning, and CLIP
//! batching.  This file is only event plumbing.

use crate::{
    config::{VECTOR_STORE_PATH, PROGRESS_EMIT_INTERVAL_MS},
    core::{database, progress, sidecar, vector_store::VectorStore},
    services::sync,
};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use once_cell::sync::{Lazy, OnceCell};
use serde_json::json;
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{channel, Sender},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter};

// ── App handle for emitting sync events to the UI ──────────────────────────
//
// The watcher runs on background threads with no access to a Tauri command's
// AppHandle, so we stash a clone here at startup.  Every sync lifecycle event
// (started / progress / complete / error) is emitted through it so the Library
// page can show live per-folder progress without polling.

static APP: OnceCell<AppHandle> = OnceCell::new();

fn emit(event: &str, payload: serde_json::Value) {
    if let Some(app) = APP.get() {
        let _ = app.emit(event, payload);
    }
}

/// Folders whose sync was requested before the CLIP model finished loading.
/// Drained by `notify_model_ready()` once the model is in memory.
static PENDING: Lazy<Mutex<HashSet<PathBuf>>> = Lazy::new(|| Mutex::new(HashSet::new()));

/// Called by the auth layer the instant the CLIP model becomes available.
/// Re-runs every watched folder (which also covers anything deferred while the
/// model was still decrypting/compiling) so nothing stays stuck on "Indexing".
pub fn notify_model_ready() {
    PENDING.lock().unwrap().clear();
    refresh_active_watches();
    reconcile_all();
}

// ── Debounce tuning ────────────────────────────────────────────────────────

/// Quiet period after the last event before we trigger a resync.
const DEBOUNCE_WINDOW: Duration = Duration::from_millis(1500);

/// Sleep granularity inside the worker loop.
const WORKER_TICK: Duration = Duration::from_millis(300);

// ── Internal channel message ───────────────────────────────────────────────

#[derive(Debug)]
enum WatcherMsg {
    /// A file-system event arrived for the given path inside a watched root.
    Event(PathBuf),
}

// ── Global state ───────────────────────────────────────────────────────────

struct WatcherState {
    /// `RecommendedWatcher` keeps OS callbacks alive while it lives.  Dropping
    /// it unregisters every path.  We rebuild it whenever the watched set
    /// changes — cheaper than tracking which path to add/remove individually.
    inner:      Option<RecommendedWatcher>,
    /// Active watch roots — only ones currently subscribed to OS events.
    active:     HashSet<PathBuf>,
    /// Sender into the worker thread.
    msg_tx:     Option<Sender<WatcherMsg>>,
}

static STATE: Lazy<Mutex<WatcherState>> = Lazy::new(|| {
    Mutex::new(WatcherState {
        inner:  None,
        active: HashSet::new(),
        msg_tx: None,
    })
});

// ── Public API ─────────────────────────────────────────────────────────────

/// Spawn the background worker thread.  Idempotent.  Call once at startup.
/// `app` is stored so sync lifecycle events can be pushed to the UI.
pub fn init(app: AppHandle) {
    let _ = APP.set(app);

    let mut state = STATE.lock().unwrap();
    if state.msg_tx.is_some() {
        return; // Already initialised.
    }

    let (tx, rx) = channel::<WatcherMsg>();
    state.msg_tx = Some(tx);
    drop(state);

    thread::spawn(move || worker_loop(rx));
    log::info!("[watcher] background worker started");
}

/// Subscribe to OS events for every watched folder recorded in the DB.
/// Safe to call multiple times — the watcher is rebuilt each time.
/// Pre-condition: the sidecar must be ready (`core::sidecar::is_ready()`), otherwise sync will fail.
pub fn refresh_active_watches() {
    let paths = match read_paths_from_db() {
        Ok(p)  => p,
        Err(e) => {
            log::warn!("[watcher] cannot read watched folders: {e}");
            return;
        }
    };

    let mut state = STATE.lock().unwrap();

    // Rebuild from scratch.  notify's underlying handle is reset by dropping.
    state.inner  = None;
    state.active = HashSet::new();

    let msg_tx = match state.msg_tx.as_ref() {
        Some(tx) => tx.clone(),
        None     => {
            log::warn!("[watcher] init() not called; cannot refresh");
            return;
        }
    };

    let event_tx = msg_tx.clone();
    let mut new_watcher = match RecommendedWatcher::new(
        move |res: notify::Result<Event>| {
            match res {
                Ok(ev) => {
                    if should_handle(&ev.kind) {
                        for p in ev.paths {
                            let _ = event_tx.send(WatcherMsg::Event(p));
                        }
                    }
                }
                Err(e) => log::warn!("[watcher] notify error: {e}"),
            }
        },
        notify::Config::default(),
    ) {
        Ok(w)  => w,
        Err(e) => {
            log::warn!("[watcher] could not create watcher: {e}");
            return;
        }
    };

    for path in &paths {
        let p = PathBuf::from(path);
        if !p.is_dir() {
            log::warn!("[watcher] skipping missing folder: {p:?}");
            continue;
        }
        match new_watcher.watch(&p, RecursiveMode::Recursive) {
            Ok(_)  => {
                log::info!("[watcher] now watching {p:?}");
                state.active.insert(p);
            }
            Err(e) => log::warn!("[watcher] failed to watch {p:?}: {e}"),
        }
    }

    state.inner = Some(new_watcher);
}

/// Trigger one full reconciliation pass on every watched folder.
/// Call after login (model loaded) to catch any changes made while the app
/// was closed.  Runs the existing `sync::sync_folder` per folder.
pub fn reconcile_all() {
    let paths = match read_paths_from_db() {
        Ok(p)  => p,
        Err(e) => { log::warn!("[watcher] reconcile: cannot read DB: {e}"); return; }
    };

    for path in paths {
        sync_one(&PathBuf::from(path));
    }

    // A full pass has re-embedded every folder — retire the migration marker so
    // subsequent launches don't re-index again.
    crate::core::migrate::clear_reembed_pending();
}

/// Reconcile a single folder.  Used by the Library service when a folder is
/// freshly added or the user clicks "Re-scan".  Re-subscribes the OS watcher
/// afterward so a previously-missing (NAS) path is picked up again.
pub fn reconcile_all_path(path: &str) {
    sync_one(&PathBuf::from(path));
    refresh_active_watches();
}

// ── NAS auto-recovery ──────────────────────────────────────────────────────

/// Parse `mount` output to find the network URL backing the given path.
/// Handles nested mounts by keeping the longest matching mount point.
/// Only compiled on macOS where `/Volumes/` NAS mounts are the concern.
#[cfg(target_os = "macos")]
fn find_network_url_for_path(path: &str) -> Option<String> {
    let output = std::process::Command::new("mount").output().ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Normalise path to have a trailing slash so prefix matching is exact.
    let path_norm = if path.ends_with('/') {
        path.to_string()
    } else {
        format!("{}/", path)
    };

    let mut best: Option<(usize, String)> = None; // (mount_point_len, url)

    for line in stdout.lines() {
        // Format: "<source> on <mountpoint> (<fstype>, ...)"
        let Some((source, rest)) = line.split_once(" on ") else { continue };
        let Some((mount_point, type_part)) = rest.split_once(" (") else { continue };
        let mount_point = mount_point.trim();

        // Skip local/system block devices.
        if source.trim().starts_with("/dev/") {
            continue;
        }

        let mp_norm = if mount_point.ends_with('/') {
            mount_point.to_string()
        } else {
            format!("{}/", mount_point)
        };

        if !path_norm.starts_with(&mp_norm) {
            continue;
        }

        // Build a mount-able URL from the source.  SMB mounts appear as
        // `//[user@]host/share`; AFP similarly.  NFS as `host:/export`.
        let source = source.trim();
        let url = if source.starts_with("//") {
            let scheme = if type_part.trim_start().starts_with("afpfs") {
                "afp:"
            } else {
                "smb:"
            };
            format!("{}{}", scheme, source)
        } else {
            continue; // unrecognised format — skip
        };

        if best.as_ref().map_or(true, |(len, _)| mount_point.len() > *len) {
            best = Some((mount_point.len(), url));
        }
    }

    best.map(|(_, url)| url)
}

/// Attempt to mount a network URL via AppleScript.  macOS uses Keychain
/// credentials automatically — no dialog unless credentials have expired.
#[cfg(target_os = "macos")]
fn try_mount_network_url(url: &str) -> bool {
    let escaped = url.replace('"', "\\\"");
    let script  = format!("mount volume \"{}\"", escaped);
    match std::process::Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
    {
        Ok(out) if out.status.success() => {
            log::info!("[watcher] mounted NAS: {url}");
            true
        }
        Ok(out) => {
            log::warn!(
                "[watcher] mount failed for {url}: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            );
            false
        }
        Err(e) => {
            log::warn!("[watcher] osascript error for {url}: {e}");
            false
        }
    }
}

/// Return the network URL (e.g. `smb://host/share`) that backs `path`, or
/// `None` if the path is local or the URL cannot be determined.
/// No-op (returns `None`) on non-macOS platforms.
pub fn capture_network_url(_path: &str) -> Option<String> {
    #[cfg(target_os = "macos")]
    { find_network_url_for_path(_path) }
    #[cfg(not(target_os = "macos"))]
    { None }
}

/// Background loop that periodically checks every "missing" folder that has a
/// stored network URL and attempts to remount it.  On success the folder
/// transitions back to "watching" without any user action.
/// No-op on non-macOS platforms.
pub fn start_nas_recovery_loop() {
    #[cfg(target_os = "macos")]
    {
        thread::spawn(|| {
            loop {
                thread::sleep(Duration::from_secs(30));

                let missing = {
                    let con = match database::open() {
                        Ok(c) => c,
                        Err(_) => continue,
                    };
                    match database::missing_folders_with_network_url(&con) {
                        Ok(m) => m,
                        Err(_) => continue,
                    }
                };

                for (path, url) in missing {
                    log::info!("[watcher] NAS recovery attempt: {path} via {url}");
                    if !try_mount_network_url(&url) {
                        continue;
                    }
                    // Give macOS a moment to settle the mount point.
                    thread::sleep(Duration::from_secs(2));
                    let p = PathBuf::from(&path);
                    if p.is_dir() {
                        log::info!("[watcher] NAS remounted — restoring {path}");
                        sync_one(&p);
                        refresh_active_watches();
                    }
                }
            }
        });
    }
}

/// On startup, populate `network_url` for any watched folder that was added
/// before this feature existed (NULL url) and is currently mounted.
/// No-op on non-macOS platforms.
pub fn backfill_network_urls() {
    #[cfg(target_os = "macos")]
    {
        thread::spawn(|| {
            let con = match database::open() {
                Ok(c) => c,
                Err(_) => return,
            };
            let paths = match database::watched_folders_without_network_url(&con) {
                Ok(p) => p,
                Err(_) => return,
            };
            for path in paths {
                if let Some(url) = find_network_url_for_path(&path) {
                    if let Err(e) = database::set_network_url(&con, &path, &url) {
                        log::warn!("[watcher] backfill URL failed for {path}: {e}");
                    } else {
                        log::info!("[watcher] backfilled network URL for {path}: {url}");
                    }
                }
            }
        });
    }
}

// ── Internals ──────────────────────────────────────────────────────────────

fn read_paths_from_db() -> crate::error::Result<Vec<String>> {
    let con = database::open()?;
    database::watched_folder_paths(&con)
}

/// Filter to events that may actually change the file set: create, content
/// modify, remove, rename.  Metadata-only changes (access time, permissions)
/// are ignored on purpose: indexing *reads* every file, which bumps its atime,
/// and macOS FSEvents reports that straight back to us — without this filter the
/// watcher re-triggers itself in an endless re-scan loop.
fn should_handle(kind: &EventKind) -> bool {
    use notify::event::ModifyKind;
    match kind {
        EventKind::Create(_) | EventKind::Remove(_) => true,
        EventKind::Modify(ModifyKind::Metadata(_))  => false,
        EventKind::Modify(_)                        => true,
        _                                           => false,
    }
}

/// Map an event path back to one of our watch roots.
fn owning_root(event_path: &PathBuf, roots: &HashSet<PathBuf>) -> Option<PathBuf> {
    for root in roots {
        if event_path.starts_with(root) {
            return Some(root.clone());
        }
    }
    None
}

/// Single-folder reconcile.  Loads the vector store, calls `sync_folder`,
/// saves the store back.  Updates the watched-folder status accordingly and
/// emits live lifecycle events to the UI:
///   library_sync_started  { path }
///   library_sync_progress { path, progress: ProgressSnapshot }
///   library_sync_complete  { path, errors, image_count }
///   library_sync_error     { path, message }
fn sync_one(folder: &PathBuf) {
    let folder_str = folder.to_string_lossy().to_string();

    if !sidecar::is_ready() {
        log::info!("[watcher] sidecar not ready; deferring sync of {folder:?}");
        PENDING.lock().unwrap().insert(folder.clone());
        if let Ok(con) = database::open() {
            let _ = database::set_watched_folder_status(&con, &folder_str, "indexing");
        }
        // Surface a "preparing" state so the card doesn't sit on a silent,
        // fake "Indexing".  notify_model_ready() will re-drive this folder
        // with real progress once the CLIP model has loaded.
        emit("library_sync_progress", json!({
            "path": folder_str,
            "progress": {
                "phase":   "Preparing AI model…",
                "done":    0,
                "total":   0,
                "percent": 0,
                "current": "",
                "errors":  0,
                "eta_sec": -1,
            }
        }));
        return;
    }
    if !folder.is_dir() {
        log::warn!("[watcher] watched folder missing: {folder:?}");
        if let Ok(con) = database::open() {
            let _ = database::set_watched_folder_status(&con, &folder_str, "missing");
        }
        emit("library_sync_error", json!({
            "path": folder_str,
            "message": "Folder no longer exists on disk.",
        }));
        return;
    }

    log::info!("[watcher] reconciling {folder:?}");
    let t_sync_one = Instant::now();

    // Serialize against the search pipeline — both load → modify → save the
    // vector store, and concurrent runs would lose updates.
    let _store_guard = crate::core::vector_store::store_io_guard();

    let t_load = Instant::now();
    let mut store = match VectorStore::load(VECTOR_STORE_PATH.as_path()) {
        Ok(s)  => {
            log::info!(
                "[timing] vector_store_load path={:?} load_ms={:.2}",
                VECTOR_STORE_PATH.as_path(),
                t_load.elapsed().as_secs_f64() * 1000.0,
            );
            s
        }
        Err(e) => {
            log::warn!("[watcher] vector store load failed: {e}");
            if let Ok(con) = database::open() {
                let _ = database::set_watched_folder_status(&con, &folder_str, "error");
            }
            emit("library_sync_error", json!({
                "path": folder_str,
                "message": format!("Could not open the index: {e}"),
            }));
            return;
        }
    };

    if let Ok(con) = database::open() {
        let _ = database::set_watched_folder_status(&con, &folder_str, "indexing");
    }

    // Clear any stale snapshot so the ticker only streams this folder's run.
    progress::reset();
    emit("library_sync_started", json!({ "path": folder_str }));

    // Spawn a ticker that streams progress snapshots to the UI while the
    // (blocking) sync runs on this thread.  Stopped via the atomic flag.
    let stop = Arc::new(AtomicBool::new(false));
    let ticker = {
        let stop      = stop.clone();
        let path_str  = folder_str.clone();
        thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                let snap = progress::get_progress();
                if snap.active {
                    emit("library_sync_progress", json!({
                        "path":     path_str,
                        "progress": snap,
                    }));
                }
                thread::sleep(Duration::from_millis(PROGRESS_EMIT_INTERVAL_MS));
            }
        })
    };

    // One-time re-index after an embedding-schema change: rebuild this folder's
    // vectors in place (keyed by existing faiss_id, so Browse tags are kept)
    // before running the normal reconcile.
    if crate::core::migrate::reembed_pending() {
        if let Err(e) = sync::reembed_folder(&mut store, folder) {
            log::warn!("[watcher] re-embed failed for {folder:?}: {e}");
        }
    }

    let sync_result = sync::sync_folder(&mut store, folder);

    // Stop the ticker before emitting the terminal event.
    stop.store(true, Ordering::Relaxed);
    let _ = ticker.join();
    progress::reset();

    match sync_result {
        Ok(errors) => {
            // Hold the write lock only for the actual file write (~2 seconds).
            // Search holds a read lock and is only blocked during this window.
            let t_save = Instant::now();
            let save_result = {
                let _write_guard = crate::core::vector_store::store_io_write_guard();
                store.save(VECTOR_STORE_PATH.as_path())
            };
            log::info!(
                "[timing] vector_store_save path={:?} save_ms={:.2}",
                VECTOR_STORE_PATH.as_path(),
                t_save.elapsed().as_secs_f64() * 1000.0,
            );
            if let Err(e) = save_result {
                log::warn!("[watcher] vector store save failed: {e}");
                if let Ok(con) = database::open() {
                    let _ = database::set_watched_folder_status(&con, &folder_str, "error");
                }
                emit("library_sync_error", json!({
                    "path": folder_str,
                    "message": format!("Could not save the index: {e}"),
                }));
                return;
            }

            let image_count = database::open()
                .ok()
                .and_then(|con| database::folder_file_count(&con, &folder_str).ok())
                .unwrap_or(0);

            if let Ok(con) = database::open() {
                let _ = database::set_watched_folder_status(&con, &folder_str, "watching");
            }

            if !errors.is_empty() {
                log::warn!("[watcher] {} per-file errors in {folder:?}:", errors.len());
                for e in &errors {
                    log::warn!("[watcher]   • {} — {}", e.file, e.reason);
                }
            }
            log::info!(
                "[timing] sync_one TOTAL folder={folder:?} wall_ms={:.2}",
                t_sync_one.elapsed().as_secs_f64() * 1000.0,
            );
            log::info!("[watcher] done reconciling {folder:?}");

            // Ship a bounded sample of the failures so the Library card can show
            // *why* files were skipped, without pushing a huge payload over IPC
            // for a pathological folder.
            const MAX_REPORTED_FAILURES: usize = 100;
            let failed: Vec<serde_json::Value> = errors
                .iter()
                .take(MAX_REPORTED_FAILURES)
                .map(|e| json!({ "file": e.file, "reason": e.reason }))
                .collect();

            emit("library_sync_complete", json!({
                "path":        folder_str,
                "errors":      errors.len(),
                "failed":      failed,
                "image_count": image_count,
            }));

            // Auto-compute colour tags for any images that don't have one yet
            // (background — never blocks the sync).  Emits `tags_updated` so the
            // Browse/filter UI can refresh its facets when colours land.
            let folder_for_color = folder_str.clone();
            std::thread::spawn(move || {
                let n = crate::services::tags::backfill_colors(&folder_for_color);
                if n > 0 {
                    emit("tags_updated", json!({ "colored": n }));
                }
            });
        }
        Err(e) => {
            log::warn!("[watcher] sync failed for {folder:?}: {e}");
            if let Ok(con) = database::open() {
                let _ = database::set_watched_folder_status(&con, &folder_str, "error");
            }
            emit("library_sync_error", json!({
                "path": folder_str,
                "message": e.to_string(),
            }));
        }
    }
}

/// Long-lived worker that owns the event channel.  Coalesces events into
/// per-folder resync requests once activity has been quiet for
/// `DEBOUNCE_WINDOW`.
fn worker_loop(rx: std::sync::mpsc::Receiver<WatcherMsg>) {
    let mut pending: HashSet<PathBuf> = HashSet::new();
    let mut last_event_at: Option<Instant> = None;

    loop {
        match rx.recv_timeout(WORKER_TICK) {
            Ok(WatcherMsg::Event(path)) => {
                let roots = {
                    let state = STATE.lock().unwrap();
                    state.active.clone()
                };
                if let Some(root) = owning_root(&path, &roots) {
                    pending.insert(root);
                    last_event_at = Some(Instant::now());
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                log::warn!("[watcher] channel disconnected; worker exiting");
                return;
            }
        }

        // Quiet-period check — once no event for DEBOUNCE_WINDOW, drain pending.
        if !pending.is_empty() {
            if let Some(t) = last_event_at {
                if t.elapsed() >= DEBOUNCE_WINDOW {
                    let to_sync: Vec<PathBuf> = pending.drain().collect();
                    last_event_at = None;
                    for folder in to_sync {
                        sync_one(&folder);
                    }
                }
            }
        }
    }
}
