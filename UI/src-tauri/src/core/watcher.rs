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
    config::VECTOR_STORE_PATH,
    core::{database, embedder, vector_store::VectorStore},
    services::sync,
};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use once_cell::sync::Lazy;
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{
        mpsc::{channel, Sender},
        Mutex,
    },
    thread,
    time::{Duration, Instant},
};

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
pub fn init() {
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
/// Pre-condition: the embedder model must be loaded, otherwise sync will fail.
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
}

/// Reconcile a single folder.  Used by the Library service when a folder is
/// freshly added or the user clicks "Re-scan".
pub fn reconcile_all_path(path: &str) {
    sync_one(&PathBuf::from(path));
}

// ── Internals ──────────────────────────────────────────────────────────────

fn read_paths_from_db() -> crate::error::Result<Vec<String>> {
    let con = database::open()?;
    database::watched_folder_paths(&con)
}

/// Filter to events that may actually change the file set: create, modify,
/// remove, rename.  Everything else (access time, attribute-only) is ignored.
fn should_handle(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    )
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
/// saves the store back.  Updates the watched-folder status accordingly.
fn sync_one(folder: &PathBuf) {
    if !embedder::is_ready() {
        log::info!("[watcher] model not ready; skipping sync of {folder:?}");
        return;
    }
    if !folder.is_dir() {
        log::warn!("[watcher] watched folder missing: {folder:?}");
        if let Ok(con) = database::open() {
            let _ = database::set_watched_folder_status(
                &con,
                &folder.to_string_lossy(),
                "missing",
            );
        }
        return;
    }

    log::info!("[watcher] reconciling {folder:?}");

    // Serialize against the search pipeline — both load → modify → save the
    // vector store, and concurrent runs would lose updates.
    let _store_guard = crate::core::vector_store::store_io_guard();

    let mut store = match VectorStore::load(VECTOR_STORE_PATH.as_path()) {
        Ok(s)  => s,
        Err(e) => { log::warn!("[watcher] vector store load failed: {e}"); return; }
    };

    if let Ok(con) = database::open() {
        let _ = database::set_watched_folder_status(
            &con,
            &folder.to_string_lossy(),
            "indexing",
        );
    }

    match sync::sync_folder(&mut store, folder) {
        Ok(errors) => {
            if !errors.is_empty() {
                log::warn!("[watcher] {} per-file errors in {folder:?}", errors.len());
            }
            if let Err(e) = store.save(VECTOR_STORE_PATH.as_path()) {
                log::warn!("[watcher] vector store save failed: {e}");
            }
            if let Ok(con) = database::open() {
                let _ = database::set_watched_folder_status(
                    &con,
                    &folder.to_string_lossy(),
                    "watching",
                );
            }
            log::info!("[watcher] done reconciling {folder:?}");
        }
        Err(e) => {
            log::warn!("[watcher] sync failed for {folder:?}: {e}");
            if let Ok(con) = database::open() {
                let _ = database::set_watched_folder_status(
                    &con,
                    &folder.to_string_lossy(),
                    "error",
                );
            }
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
