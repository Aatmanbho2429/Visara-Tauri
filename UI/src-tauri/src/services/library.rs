//! Watched-folder library management.
//!
//! Thin facade over `database` + `watcher`.  Each public function maps 1:1 to
//! a Tauri command and returns a `serde_json::Value` suitable for the Angular
//! `BaseResponse<T>` envelope.

use crate::{
    core::{database, watcher},
    error::Result,
};
use serde_json::{json, Value};
use std::path::PathBuf;

/// Return the full list of watched folders for the Library page.
pub fn list_folders() -> Value {
    match list_folders_inner() {
        Ok(folders) => json!({
            "success": true,
            "message": "ok",
            "data":    { "folders": folders },
        }),
        Err(e) => err(format!("Could not load folders: {e}")),
    }
}

fn list_folders_inner() -> Result<Vec<database::WatchedFolder>> {
    let con = database::open()?;
    database::list_watched_folders(&con)
}

/// Snapshot used by Angular to drive the migration banner and search empty
/// states.  Returns watched-folder count plus total indexed files.
pub fn stats() -> Value {
    match stats_inner() {
        Ok((wf_count, total_files)) => json!({
            "success": true,
            "message": "ok",
            "data": {
                "watched_folder_count": wf_count,
                "total_indexed_files":  total_files,
            },
        }),
        Err(e) => err(format!("Could not read stats: {e}")),
    }
}

fn stats_inner() -> Result<(usize, usize)> {
    let con = database::open()?;
    let folders = database::list_watched_folders(&con)?;
    let total   = database::total_indexed_files(&con)?;
    Ok((folders.len(), total))
}

/// Add a folder to the watched set, refresh the OS watcher, and trigger an
/// initial reconciliation in the background.
pub fn add_folder(path: String) -> Value {
    let p = PathBuf::from(&path);

    if !p.is_dir() {
        return err(format!("Folder does not exist: {path}"));
    }

    let con = match database::open() {
        Ok(c)  => c,
        Err(e) => return err(format!("Database error: {e}")),
    };

    if let Err(e) = database::insert_watched_folder(&con, &path) {
        return err(format!("Could not save folder: {e}"));
    }

    // Refresh OS subscriptions to include the new folder.
    watcher::refresh_active_watches();

    // Kick off the initial scan on a worker thread so the UI returns now.
    let path_for_task = path.clone();
    std::thread::spawn(move || {
        log::info!("[library] initial reconcile of newly-added folder: {path_for_task}");
        watcher::reconcile_all_path(&path_for_task);
    });

    json!({
        "success": true,
        "message": "Folder added — indexing will run in the background.",
        "data":    null,
    })
}

/// Remove a folder from the watched set.  The existing embeddings stay in
/// `vectors.bin` + `meta.db` so subsequent re-adds reuse them.  Set
/// `purge = true` to also delete every row whose path is under the folder.
pub fn remove_folder(path: String, purge: bool) -> Value {
    let con = match database::open() {
        Ok(c)  => c,
        Err(e) => return err(format!("Database error: {e}")),
    };

    if let Err(e) = database::delete_watched_folder(&con, &path) {
        return err(format!("Could not remove folder: {e}"));
    }

    if purge {
        // Nuke every row under this folder.  Vector tombstones will be
        // reclaimed on the next compaction.
        if let Err(e) = purge_folder_rows(&con, &path) {
            log::warn!("[library] purge failed for {path}: {e}");
        }
    }

    watcher::refresh_active_watches();

    json!({
        "success": true,
        "message": "Folder removed.",
        "data":    null,
    })
}

/// Pause / resume.  Paused folders stay in the index but receive no OS events.
pub fn set_paused(path: String, paused: bool) -> Value {
    let con = match database::open() {
        Ok(c)  => c,
        Err(e) => return err(format!("Database error: {e}")),
    };

    let new_status = if paused { "paused" } else { "watching" };
    if let Err(e) = database::set_watched_folder_status(&con, &path, new_status) {
        return err(format!("Could not update folder: {e}"));
    }

    watcher::refresh_active_watches();

    json!({
        "success": true,
        "message": if paused { "Folder paused." } else { "Folder resumed." },
        "data":    null,
    })
}

/// Force a re-scan in the background.
pub fn rescan_folder(path: String) -> Value {
    let path_for_task = path.clone();
    std::thread::spawn(move || {
        watcher::reconcile_all_path(&path_for_task);
    });
    json!({
        "success": true,
        "message": "Re-scan started.",
        "data":    null,
    })
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn purge_folder_rows(con: &rusqlite::Connection, folder: &str) -> rusqlite::Result<usize> {
    let prefix = if folder.ends_with(std::path::MAIN_SEPARATOR) {
        folder.to_string()
    } else {
        format!("{folder}{}", std::path::MAIN_SEPARATOR)
    };
    con.execute(
        "DELETE FROM files WHERE path LIKE ?1",
        rusqlite::params![format!("{prefix}%")],
    )
}

fn err(msg: String) -> Value {
    json!({ "success": false, "message": msg, "data": null })
}
