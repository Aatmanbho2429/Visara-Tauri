//! SQLite wrapper — image-path ↔ vector-ID metadata store.
//!
//! Schema is intentionally kept identical to the legacy Python schema so that
//! existing databases are automatically reused after migration.

use crate::{config, error::Result};
use rusqlite::{Connection, params};
use std::{collections::HashSet, fs, path::Path};

// ── Connection ────────────────────────────────────────────────────────────

/// Open (and migrate) the SQLite database.  Call once per worker thread;
/// SQLite connections are not Send so never share one across threads.
pub fn open() -> Result<Connection> {
    if let Some(parent) = config::DB_PATH.parent() {
        fs::create_dir_all(parent)?;
    }

    let con = Connection::open(config::DB_PATH.as_path())?;

    con.execute_batch("
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous  = NORMAL;
        PRAGMA foreign_keys = ON;
    ")?;

    con.execute_batch("
        CREATE TABLE IF NOT EXISTS files (
            id       INTEGER PRIMARY KEY AUTOINCREMENT,
            path     TEXT    UNIQUE NOT NULL,
            hash     TEXT    NOT NULL,
            faiss_id INTEGER UNIQUE NOT NULL,
            mtime    REAL    NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_hash     ON files(hash);
        CREATE INDEX IF NOT EXISTS idx_path     ON files(path);
        CREATE INDEX IF NOT EXISTS idx_faiss_id ON files(faiss_id);
    ")?;

    // Idempotent column addition for databases created before mtime existed.
    let _ = con.execute("ALTER TABLE files ADD COLUMN mtime REAL NOT NULL DEFAULT 0", []);

    Ok(con)
}

// ── Queries ───────────────────────────────────────────────────────────────

/// Return `(path, vector_id)` for any row whose hash matches, or `(None, None)`.
pub fn find_by_hash(con: &Connection, hash: &str) -> Result<(Option<String>, Option<i64>)> {
    let mut stmt = con.prepare_cached(
        "SELECT path, faiss_id FROM files WHERE hash = ? LIMIT 1",
    )?;
    let row = stmt.query_row(params![hash], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)));
    match row {
        Ok((path, id)) => Ok((Some(path), Some(id))),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok((None, None)),
        Err(e) => Err(e.into()),
    }
}

/// Return `(vector_id, hash, mtime)` for a path, or `None`.
pub fn find_by_path(con: &Connection, path: &str) -> Result<Option<(i64, String, f64)>> {
    let mut stmt = con.prepare_cached(
        "SELECT faiss_id, hash, mtime FROM files WHERE path = ? LIMIT 1",
    )?;
    let row = stmt.query_row(params![path], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, f64>(2)?))
    });
    match row {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Next available vector ID (max + 1, or 0 if table is empty).
pub fn next_vector_id(con: &Connection) -> Result<i64> {
    let max: Option<i64> =
        con.query_row("SELECT MAX(faiss_id) FROM files", [], |r| r.get(0))?;
    Ok(max.map(|m| m + 1).unwrap_or(0))
}

pub fn insert_file(
    con:       &Connection,
    path:      &str,
    hash:      &str,
    vector_id: i64,
    mtime:     f64,
) -> Result<()> {
    con.execute(
        "INSERT OR REPLACE INTO files (path, hash, faiss_id, mtime) VALUES (?1,?2,?3,?4)",
        params![path, hash, vector_id, mtime],
    )?;
    Ok(())
}

pub fn move_file(con: &Connection, old_path: &str, new_path: &str) -> Result<()> {
    con.execute(
        "UPDATE files SET path = ?1 WHERE path = ?2",
        params![new_path, old_path],
    )?;
    Ok(())
}

pub fn delete_file(con: &Connection, path: &str) -> Result<()> {
    con.execute("DELETE FROM files WHERE path = ?1", params![path])?;
    Ok(())
}

/// Remove DB rows for files no longer on disk within a folder.
/// Returns the vector IDs of removed rows so the caller can purge the store.
pub fn cleanup_missing_in_folder(con: &Connection, folder: &str) -> Result<Vec<i64>> {
    let prefix = normalise_folder_prefix(folder);
    let mut stmt = con.prepare_cached(
        "SELECT path, faiss_id FROM files WHERE path LIKE ?1",
    )?;
    let rows: Vec<(String, i64)> = stmt
        .query_map(params![format!("{prefix}%")], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })?
        .filter_map(|r| r.ok())
        .collect();

    let mut removed_ids = Vec::new();
    for (path, id) in rows {
        if !Path::new(&path).exists() {
            con.execute("DELETE FROM files WHERE path = ?1", params![path])?;
            removed_ids.push(id);
        }
    }
    Ok(removed_ids)
}

/// `(vector_id → path)` map for every file in a folder — used by search.
pub fn folder_id_map(con: &Connection, folder: &str) -> Result<std::collections::HashMap<i64, String>> {
    let prefix = normalise_folder_prefix(folder);
    let mut stmt = con.prepare_cached(
        "SELECT faiss_id, path FROM files WHERE path LIKE ?1",
    )?;
    let map = stmt
        .query_map(params![format!("{prefix}%")], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(map)
}

/// Set of all hashes for files within a folder — used by cleanup phase.
pub fn folder_hashes(con: &Connection, folder: &str) -> Result<HashSet<String>> {
    let prefix = normalise_folder_prefix(folder);
    let mut stmt = con.prepare_cached(
        "SELECT hash FROM files WHERE path LIKE ?1",
    )?;
    let set = stmt
        .query_map(params![format!("{prefix}%")], |r| r.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(set)
}

/// `(path, vector_id)` pairs for a set of hashes.
pub fn files_by_hashes(con: &Connection, hashes: &HashSet<String>) -> Result<Vec<(String, i64)>> {
    if hashes.is_empty() {
        return Ok(Vec::new());
    }
    // Build a parameterised IN clause dynamically.
    let placeholders: String = (0..hashes.len()).map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!("SELECT path, faiss_id FROM files WHERE hash IN ({placeholders})");
    let mut stmt = con.prepare(&sql)?;
    let hash_vec: Vec<&String> = hashes.iter().collect();
    let pairs = stmt
        .query_map(rusqlite::params_from_iter(hash_vec.iter()), |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(pairs)
}

/// Row count for a folder — used to decide whether a sync is needed.
pub fn folder_file_count(con: &Connection, folder: &str) -> Result<usize> {
    let prefix = normalise_folder_prefix(folder);
    let count: i64 = con.query_row(
        "SELECT COUNT(*) FROM files WHERE path LIKE ?1",
        params![format!("{prefix}%")],
        |r| r.get(0),
    )?;
    Ok(count as usize)
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn normalise_folder_prefix(folder: &str) -> String {
    let normalised = std::path::Path::new(folder)
        .to_string_lossy()
        .to_string();
    if normalised.ends_with(std::path::MAIN_SEPARATOR) {
        normalised
    } else {
        format!("{normalised}{}", std::path::MAIN_SEPARATOR)
    }
}
