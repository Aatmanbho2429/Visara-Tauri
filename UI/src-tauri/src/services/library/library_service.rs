// Watched-folder library management.
//
// Thin facade over `database` + `watcher`.  Each public function maps 1:1 to
// a Tauri command and returns an `ApiResponse<T>` for the Angular envelope.

use crate::{
    core::{database, watcher},
    error::Result,
    models::response::{ApiResponse, ResponseFolderTree, ResponseLibraryStats, ResponseListFolders},
};
use std::path::PathBuf;

// Return the full list of watched folders for the Library page.
pub fn list_folders() -> ApiResponse<ResponseListFolders> {
    match list_folders_inner() {
        Ok(folders) => ApiResponse::ok(ResponseListFolders { folders }),
        Err(e) => ApiResponse::err(500, format!("Could not load folders: {e}")),
    }
}

fn list_folders_inner() -> Result<Vec<database::WatchedFolder>> {
    let con = database::open()?;
    database::list_watched_folders(&con)
}

// Snapshot used by Angular to drive the migration banner and search empty
// states.  Returns watched-folder count plus total indexed files.
pub fn stats() -> ApiResponse<ResponseLibraryStats> {
    match stats_inner() {
        Ok((watched_folder_count, total_indexed_files)) => {
            ApiResponse::ok(ResponseLibraryStats { watched_folder_count, total_indexed_files })
        }
        Err(e) => ApiResponse::err(500, format!("Could not read stats: {e}")),
    }
}

fn stats_inner() -> Result<(usize, usize)> {
    let con = database::open()?;
    let folders = database::list_watched_folders(&con)?;
    let total   = database::total_indexed_files(&con)?;
    Ok((folders.len(), total))
}

// Add a folder to the watched set, refresh the OS watcher, and trigger an
// initial reconciliation in the background.
pub fn add_folder(path: String) -> ApiResponse<()> {
    let p = PathBuf::from(&path);

    if !p.is_dir() {
        return ApiResponse::err(404, format!("Folder does not exist: {path}"));
    }

    let con = match database::open() {
        Ok(c)  => c,
        Err(e) => return ApiResponse::err(500, format!("Database error: {e}")),
    };

    // Detect overlaps with already-watched folders.  A recursive watch on a
    // parent already covers its children, so two overlapping watches would
    // double-count and double-scan every shared file.
    let new_dir = norm_dir(&path);
    let mut absorbed: Vec<String> = Vec::new();
    if let Ok(existing) = database::list_watched_folders(&con) {
        for e in &existing {
            let e_dir = norm_dir(&e.path);
            if e_dir == new_dir {
                return ApiResponse::err(409, "That folder is already in your Library.");
            }
            if new_dir.starts_with(&e_dir) {
                // New folder sits inside an existing watched parent → it is
                // already covered recursively, so there is nothing to add.
                return ApiResponse::err(409, format!(
                    "This folder is already covered by a watched parent folder:\n{}\n\nSubfolders are indexed automatically — no need to add them separately.",
                    e.path
                ));
            }
            if e_dir.starts_with(&new_dir) {
                // An existing watched folder sits inside the new one → it
                // becomes redundant once the parent is watched recursively.
                absorbed.push(e.path.clone());
            }
        }
    }

    // Absorb the redundant child folders: drop their watched-folder rows but
    // KEEP their embeddings.  Those files now fall under the new parent's scope,
    // so nothing has to be re-indexed — the parent's reconcile reuses them
    // (their mtime is unchanged, so the scan skips straight past).
    for child in &absorbed {
        match database::delete_watched_folder(&con, child) {
            Ok(_)  => log::info!("[library] absorbed watched subfolder '{child}' into '{path}'"),
            Err(e) => log::warn!("[library] could not absorb child '{child}': {e}"),
        }
    }

    if let Err(e) = database::insert_watched_folder(&con, &path) {
        return ApiResponse::err(500, format!("Could not save folder: {e}"));
    }

    // Refresh OS subscriptions to include the new folder.
    watcher::refresh_active_watches();

    // If this is a network mount (NAS), capture the backing URL so the app
    // can remount it automatically if it disconnects later.
    if let Some(url) = watcher::capture_network_url(&path) {
        if let Ok(con) = database::open() {
            let _ = database::set_network_url(&con, &path, &url);
            log::info!("[library] stored network URL for {path}: {url}");
        }
    }

    // Kick off the initial scan on a worker thread so the UI returns now.
    let path_for_task = path.clone();
    std::thread::spawn(move || {
        log::info!("[library] initial reconcile of newly-added folder: {path_for_task}");
        watcher::reconcile_all_path(&path_for_task);
    });

    let message = if absorbed.is_empty() {
        "Folder added — indexing will run in the background.".to_string()
    } else {
        format!(
            "Folder added — {} watched subfolder{} merged into it. Indexing runs in the background.",
            absorbed.len(),
            if absorbed.len() == 1 { "" } else { "s" },
        )
    };

    ApiResponse::ok_empty(message)
}

// Remove a folder from the watched set.  The existing embeddings stay in
// `vectors.bin` + `meta.db` so subsequent re-adds reuse them.  Set
// `purge = true` to also delete every row whose path is under the folder.
pub fn remove_folder(path: String, purge: bool) -> ApiResponse<()> {
    let con = match database::open() {
        Ok(c)  => c,
        Err(e) => return ApiResponse::err(500, format!("Database error: {e}")),
    };

    if let Err(e) = database::delete_watched_folder(&con, &path) {
        return ApiResponse::err(500, format!("Could not remove folder: {e}"));
    }

    if purge {
        // Nuke every row under this folder.  Vector tombstones will be
        // reclaimed on the next compaction.
        if let Err(e) = purge_folder_rows(&con, &path) {
            log::warn!("[library] purge failed for {path}: {e}");
        }
    }

    watcher::refresh_active_watches();

    ApiResponse::ok_empty("Folder removed.")
}

// Pause / resume.  Paused folders stay in the index but receive no OS events.
pub fn set_paused(path: String, paused: bool) -> ApiResponse<()> {
    let con = match database::open() {
        Ok(c)  => c,
        Err(e) => return ApiResponse::err(500, format!("Database error: {e}")),
    };

    let new_status = if paused { "paused" } else { "watching" };
    if let Err(e) = database::set_watched_folder_status(&con, &path, new_status) {
        return ApiResponse::err(500, format!("Could not update folder: {e}"));
    }

    watcher::refresh_active_watches();

    ApiResponse::ok_empty(if paused { "Folder paused." } else { "Folder resumed." })
}

// Nested subfolder tree (with image counts) for one watched folder.  Used by
// the Library page's "View subfolders" panel.
pub fn folder_tree(path: String) -> ApiResponse<ResponseFolderTree> {
    match folder_tree_inner(&path) {
        Ok(tree) => ApiResponse::ok(ResponseFolderTree { tree }),
        Err(e) => ApiResponse::err(500, format!("Could not read subfolders: {e}")),
    }
}

fn folder_tree_inner(path: &str) -> Result<database::FolderTreeNode> {
    let con = database::open()?;
    database::folder_tree(&con, path)
}

// Force a re-scan in the background.
pub fn rescan_folder(path: String) -> ApiResponse<()> {
    let path_for_task = path.clone();
    std::thread::spawn(move || {
        watcher::reconcile_all_path(&path_for_task);
    });
    ApiResponse::ok_empty("Re-scan started.")
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn purge_folder_rows(con: &rusqlite::Connection, folder: &str) -> rusqlite::Result<usize> {
    let prefix = if folder.ends_with(std::path::MAIN_SEPARATOR) {
        folder.to_string()
    } else {
        format!("{folder}{}", std::path::MAIN_SEPARATOR)
    };
    // Deleting the file rows cascades to their tags (file_tags → files, ON
    // DELETE CASCADE), so Browse facets update automatically — no separate tag
    // delete needed.  The connection is opened via database::open(), which sets
    // PRAGMA foreign_keys = ON, so the cascade actually fires.
    con.execute(
        "DELETE FROM files WHERE path LIKE ?1",
        rusqlite::params![format!("{prefix}%")],
    )
}

// Normalise a folder path to a directory prefix with exactly one trailing
// separator, so prefix `starts_with` checks classify nesting correctly:
// `/a/b/` starts_with `/a/` ⇒ b is inside a, while `/a-b/` does not.
fn norm_dir(p: &str) -> String {
    let trimmed = p.trim_end_matches(std::path::MAIN_SEPARATOR);
    format!("{trimmed}{}", std::path::MAIN_SEPARATOR)
}
