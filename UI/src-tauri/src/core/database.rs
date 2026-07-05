//! SQLite wrapper — image-path ↔ vector-ID metadata store.
//!
//! Schema is intentionally kept identical to the legacy Python schema so that
//! existing databases are automatically reused after migration.

use crate::{config, error::Result};
use rusqlite::{Connection, params};
use std::{collections::{HashMap, HashSet}, fs, path::Path};

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

        CREATE TABLE IF NOT EXISTS watched_folders (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            path          TEXT    UNIQUE NOT NULL,
            status        TEXT    NOT NULL DEFAULT 'watching',
            added_at      REAL    NOT NULL,
            last_event_at REAL    NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_watched_path ON watched_folders(path);

        CREATE TABLE IF NOT EXISTS file_tags (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            file_id    INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
            category   TEXT NOT NULL,   -- color | material | finish | size | design | collection | custom
            value      TEXT NOT NULL,
            source     TEXT NOT NULL DEFAULT 'manual',  -- auto | manual | filename
            created_at REAL NOT NULL DEFAULT 0,
            UNIQUE(file_id, category, value)
        );
        -- NOTE: the file_id index is created by migrate_file_tags_to_file_id(),
        -- NOT here: on a legacy database file_tags still has the old path-based
        -- schema with no file_id column, so an index on it would fail this whole
        -- batch (and block the very migration that fixes it).  idx on
        -- (category, value) is safe — those columns exist in both schemas.
        CREATE INDEX IF NOT EXISTS idx_file_tags_cat_val ON file_tags(category, value);

        CREATE TABLE IF NOT EXISTS catalog_themes (
            id         TEXT PRIMARY KEY,
            name       TEXT NOT NULL,
            json       TEXT NOT NULL,   -- serialized document model (pages + elements)
            updated_at REAL NOT NULL DEFAULT 0
        );
    ")?;

    // Idempotent column addition for databases created before mtime existed.
    let _ = con.execute("ALTER TABLE files ADD COLUMN mtime REAL NOT NULL DEFAULT 0", []);

    Ok(con)
}

// ── Schema / embedding version helpers ─────────────────────────────────────

/// SQLite `PRAGMA user_version` — repurposed as the *embedding schema* version so
/// a change to how vectors are produced (model / preprocessing / colour) can be
/// detected on startup and turned into a one-time re-index.
pub fn user_version(con: &Connection) -> Result<i64> {
    Ok(con.query_row("PRAGMA user_version", [], |r| r.get(0))?)
}

pub fn set_user_version(con: &Connection, v: i64) -> Result<()> {
    // PRAGMA does not accept bound parameters; v is a trusted integer constant.
    con.execute_batch(&format!("PRAGMA user_version = {v};"))?;
    Ok(())
}

/// One-time schema upgrade: convert the legacy path-keyed `file_tags` table to
/// reference `files(id)` with `ON DELETE CASCADE`.  Idempotent — it detects the
/// old `path` column and rebuilds, copying tags across by joining on the file
/// path.  Orphan tags (whose path is no longer in `files`) are dropped by the
/// join, which is exactly the cleanup the old `sweep_orphan_tags` hack did.
/// Call once at startup, before any tag read/write.
pub fn migrate_file_tags_to_file_id(con: &Connection) -> Result<()> {
    let has_path = {
        let mut stmt = con.prepare("PRAGMA table_info(file_tags)")?;
        let cols = stmt
            .query_map([], |r| r.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .collect::<Vec<String>>();
        cols.iter().any(|c| c == "path")
    };
    if !has_path {
        // Already migrated, or a fresh install whose file_tags was created on the
        // new schema by `open()` (which deliberately does not build the file_id
        // index).  Ensure that index exists, then we're done.
        con.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_file_tags_file ON file_tags(file_id);",
        )?;
        return Ok(());
    }

    log::info!("[db] migrating file_tags to file_id-based schema…");
    con.execute_batch(
        "PRAGMA foreign_keys = OFF;
         BEGIN;
         CREATE TABLE file_tags_new (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            file_id    INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
            category   TEXT NOT NULL,
            value      TEXT NOT NULL,
            source     TEXT NOT NULL DEFAULT 'manual',
            created_at REAL NOT NULL DEFAULT 0,
            UNIQUE(file_id, category, value)
         );
         INSERT OR IGNORE INTO file_tags_new (file_id, category, value, source, created_at)
            SELECT f.id, t.category, t.value, t.source, t.created_at
            FROM file_tags t JOIN files f ON f.path = t.path;
         DROP TABLE file_tags;
         ALTER TABLE file_tags_new RENAME TO file_tags;
         CREATE INDEX IF NOT EXISTS idx_file_tags_file    ON file_tags(file_id);
         CREATE INDEX IF NOT EXISTS idx_file_tags_cat_val ON file_tags(category, value);
         COMMIT;
         PRAGMA foreign_keys = ON;",
    )?;
    log::info!("[db] file_tags migration complete");
    Ok(())
}

/// Cleanup of `file_tags` rows whose file no longer exists in `files`.  With the
/// `ON DELETE CASCADE` foreign key these should never accumulate, but this stays
/// as a cheap belt-and-suspenders sweep for rows left by pre-migration builds.
/// Returns the number of orphaned tag rows removed.
pub fn sweep_orphan_tags(con: &Connection) -> Result<usize> {
    let removed = con.execute(
        "DELETE FROM file_tags WHERE file_id NOT IN (SELECT id FROM files)",
        [],
    )?;
    Ok(removed)
}

// ── Watched folders queries ───────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct WatchedFolder {
    pub id:            i64,
    pub path:          String,
    pub status:        String,
    pub added_at:      f64,
    pub last_event_at: f64,
    pub image_count:   usize,
}

pub fn list_watched_folders(con: &Connection) -> Result<Vec<WatchedFolder>> {
    let mut stmt = con.prepare_cached(
        "SELECT id, path, status, added_at, last_event_at FROM watched_folders ORDER BY added_at DESC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, f64>(3)?,
                r.get::<_, f64>(4)?,
            ))
        })?
        .filter_map(|r| r.ok())
        .collect::<Vec<_>>();

    let mut out = Vec::with_capacity(rows.len());
    for (id, path, status, added_at, last_event_at) in rows {
        let image_count = folder_file_count(con, &path).unwrap_or(0);
        out.push(WatchedFolder { id, path, status, added_at, last_event_at, image_count });
    }
    Ok(out)
}

pub fn watched_folder_paths(con: &Connection) -> Result<Vec<String>> {
    let mut stmt = con.prepare_cached(
        "SELECT path FROM watched_folders WHERE status != 'paused' ORDER BY added_at DESC",
    )?;
    let v = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(v)
}

pub fn insert_watched_folder(con: &Connection, path: &str) -> Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    con.execute(
        "INSERT OR IGNORE INTO watched_folders (path, status, added_at, last_event_at) \
         VALUES (?1, 'indexing', ?2, ?2)",
        params![path, now],
    )?;
    Ok(())
}

pub fn delete_watched_folder(con: &Connection, path: &str) -> Result<()> {
    con.execute("DELETE FROM watched_folders WHERE path = ?1", params![path])?;
    Ok(())
}

pub fn set_watched_folder_status(con: &Connection, path: &str, status: &str) -> Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    con.execute(
        "UPDATE watched_folders SET status = ?1, last_event_at = ?2 WHERE path = ?3",
        params![status, now, path],
    )?;
    Ok(())
}

/// Total rows in `files` — used to detect legacy users with an existing index
/// but no watched folders registered yet.
pub fn total_indexed_files(con: &Connection) -> Result<usize> {
    let count: i64 = con.query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))?;
    Ok(count as usize)
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
    // Tags reference files(id), which is stable across a rename — updating the
    // path alone keeps every tag attached automatically.
    con.execute(
        "UPDATE files SET path = ?1 WHERE path = ?2",
        params![new_path, old_path],
    )?;
    Ok(())
}

pub fn delete_file(con: &Connection, path: &str) -> Result<()> {
    // ON DELETE CASCADE purges the file's tags automatically.
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
            // ON DELETE CASCADE removes this file's tags with the row.
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

// ── Subfolder tree ─────────────────────────────────────────────────────────

/// One node in a watched folder's subfolder tree.  `direct` = images stored
/// directly in this folder; `total` = images in this folder and all descendants.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FolderTreeNode {
    pub name:     String,
    pub rel:      String,
    pub direct:   usize,
    pub total:    usize,
    pub children: Vec<FolderTreeNode>,
}

/// Build the nested subfolder tree (with per-folder image counts) for a watched
/// root, derived entirely from the indexed file paths already in `files`.
pub fn folder_tree(con: &Connection, root: &str) -> Result<FolderTreeNode> {
    let prefix = normalise_folder_prefix(root);

    let mut stmt = con.prepare_cached("SELECT path FROM files WHERE path LIKE ?1")?;
    let paths: Vec<String> = stmt
        .query_map(params![format!("{prefix}%")], |r| r.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();

    let sep = std::path::MAIN_SEPARATOR;
    let mut direct: HashMap<String, usize> = HashMap::new();
    let mut total:  HashMap<String, usize> = HashMap::new();

    for p in &paths {
        // Path relative to the watched root (root prefix stripped).
        let rel = match p.get(prefix.len()..) {
            Some(r) => r,
            None    => continue,
        };
        let segs: Vec<&str> = rel.split(sep).filter(|s| !s.is_empty()).collect();
        if segs.is_empty() { continue; }

        // All but the last segment are directories; the last is the file name.
        let dir_segs = &segs[..segs.len() - 1];
        *direct.entry(dir_segs.join("/")).or_insert(0) += 1;

        // total: the root and every ancestor directory along the chain.
        *total.entry(String::new()).or_insert(0) += 1;
        let mut acc = String::new();
        for seg in dir_segs {
            if acc.is_empty() { acc = (*seg).to_string(); }
            else              { acc = format!("{acc}/{seg}"); }
            *total.entry(acc.clone()).or_insert(0) += 1;
        }
    }

    // Parent → children adjacency over every directory key.
    let mut children_map: HashMap<String, Vec<String>> = HashMap::new();
    for key in total.keys() {
        if key.is_empty() { continue; }
        let parent = match key.rsplit_once('/') {
            Some((p, _)) => p.to_string(),
            None         => String::new(),
        };
        children_map.entry(parent).or_default().push(key.clone());
    }

    let root_name = Path::new(root)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(root)
        .to_string();

    Ok(build_tree_node("", &root_name, &children_map, &direct, &total))
}

fn build_tree_node(
    key:          &str,
    root_name:    &str,
    children_map: &HashMap<String, Vec<String>>,
    direct:       &HashMap<String, usize>,
    total:        &HashMap<String, usize>,
) -> FolderTreeNode {
    let name = if key.is_empty() {
        root_name.to_string()
    } else {
        key.rsplit('/').next().unwrap_or(key).to_string()
    };

    let mut children: Vec<FolderTreeNode> = children_map
        .get(key)
        .map(|ks| {
            ks.iter()
                .map(|k| build_tree_node(k, root_name, children_map, direct, total))
                .collect()
        })
        .unwrap_or_default();

    // Most-populated subfolders first, then alphabetical.
    children.sort_by(|a, b| b.total.cmp(&a.total).then_with(|| a.name.cmp(&b.name)));

    FolderTreeNode {
        name,
        rel:    key.to_string(),
        direct: *direct.get(key).unwrap_or(&0),
        total:  *total.get(key).unwrap_or(&0),
        children,
    }
}

// ── Tags ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct FileTag {
    pub path:     String,
    pub category: String,
    pub value:    String,
    pub source:   String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TagFacet {
    pub category: String,
    pub value:    String,
    pub count:    usize,
}

fn now_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// Add a tag (no-op if the exact (file, category, value) already exists, or if
/// the path is not an indexed file).
pub fn add_tag(con: &Connection, path: &str, category: &str, value: &str, source: &str) -> Result<()> {
    con.execute(
        "INSERT OR IGNORE INTO file_tags (file_id, category, value, source, created_at) \
         SELECT id, ?2, ?3, ?4, ?5 FROM files WHERE path = ?1",
        params![path, category, value, source, now_secs()],
    )?;
    Ok(())
}

/// Replace every value in a single-valued category for a path (color, size,
/// material, finish, design).
pub fn set_single_tag(con: &Connection, path: &str, category: &str, value: &str, source: &str) -> Result<()> {
    con.execute(
        "DELETE FROM file_tags WHERE category = ?2 \
           AND file_id = (SELECT id FROM files WHERE path = ?1)",
        params![path, category],
    )?;
    add_tag(con, path, category, value, source)
}

pub fn remove_tag(con: &Connection, path: &str, category: &str, value: &str) -> Result<()> {
    con.execute(
        "DELETE FROM file_tags WHERE category = ?2 AND value = ?3 \
           AND file_id = (SELECT id FROM files WHERE path = ?1)",
        params![path, category, value],
    )?;
    Ok(())
}

pub fn path_has_category(con: &Connection, path: &str, category: &str) -> Result<bool> {
    let n: i64 = con.query_row(
        "SELECT COUNT(*) FROM file_tags t JOIN files f ON f.id = t.file_id \
          WHERE f.path = ?1 AND t.category = ?2",
        params![path, category],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

/// All tags for a set of paths.
pub fn tags_for_paths(con: &Connection, paths: &[String]) -> Result<Vec<FileTag>> {
    if paths.is_empty() { return Ok(Vec::new()); }
    let placeholders = (0..paths.len()).map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT f.path, t.category, t.value, t.source \
           FROM file_tags t JOIN files f ON f.id = t.file_id \
          WHERE f.path IN ({placeholders}) ORDER BY t.category, t.value"
    );
    let mut stmt = con.prepare(&sql)?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(paths.iter()), |r| {
            Ok(FileTag {
                path:     r.get(0)?,
                category: r.get(1)?,
                value:    r.get(2)?,
                source:   r.get(3)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

/// Distinct (category, value) with a count of how many files carry each — drives
/// the filter chips.
pub fn tag_facets(con: &Connection) -> Result<Vec<TagFacet>> {
    let mut stmt = con.prepare(
        "SELECT category, value, COUNT(DISTINCT file_id) FROM file_tags \
         GROUP BY category, value ORDER BY category ASC, COUNT(DISTINCT file_id) DESC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(TagFacet {
                category: r.get(0)?,
                value:    r.get(1)?,
                count:    r.get::<_, i64>(2)? as usize,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

/// Paths matching the given (category, value) filters. Filters in the *same*
/// category are OR'd together (e.g. color = "Light Grey" OR "Beige" OR "Green"
/// shows tiles in any of those colors), while different categories are AND'd
/// (e.g. color must match AND finish must match). With no filters, returns
/// every indexed image (capped) — used by the catalog editor's image-library
/// panel to show the whole library.
pub fn query_paths_by_tags(con: &Connection, filters: &[(String, String)]) -> Result<Vec<String>> {
    if filters.is_empty() {
        let mut stmt = con.prepare("SELECT path FROM files ORDER BY path LIMIT 5000")?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();
        return Ok(rows);
    }

    let clause = (0..filters.len())
        .map(|_| "(t.category = ? AND t.value = ?)")
        .collect::<Vec<_>>()
        .join(" OR ");
    // A path qualifies once it has at least one matching tag in EVERY distinct
    // category requested — i.e. AND across categories, OR within a category.
    // The required count is the number of *distinct* categories — a trusted
    // integer, inlined directly. Binding it as a parameter makes it TEXT, and
    // SQLite's `COUNT(*) = '1'` (integer vs text) is always false, which
    // silently returned zero rows.
    let distinct_categories = filters.iter().map(|(c, _)| c.as_str()).collect::<HashSet<_>>().len();
    let sql = format!(
        "SELECT f.path FROM file_tags t JOIN files f ON f.id = t.file_id \
          WHERE {clause} GROUP BY t.file_id HAVING COUNT(DISTINCT t.category) = {}",
        distinct_categories
    );

    let mut stmt = con.prepare(&sql)?;
    let mut binds: Vec<&str> = Vec::with_capacity(filters.len() * 2);
    for (c, v) in filters {
        binds.push(c.as_str());
        binds.push(v.as_str());
    }

    let rows = stmt
        .query_map(rusqlite::params_from_iter(binds), |r| r.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

/// Files under a folder that have no tag in `category` yet (used by the color
/// backfill so we never re-decode an already-coloured image).
pub fn paths_missing_category_in_folder(con: &Connection, folder: &str, category: &str) -> Result<Vec<String>> {
    let prefix = normalise_folder_prefix(folder);
    let mut stmt = con.prepare(
        "SELECT f.path FROM files f \
         WHERE f.path LIKE ?1 \
           AND NOT EXISTS (SELECT 1 FROM file_tags t WHERE t.file_id = f.id AND t.category = ?2)",
    )?;
    let rows = stmt
        .query_map(params![format!("{prefix}%"), category], |r| r.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

// ── Catalog themes ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct ThemeSummary {
    pub id:         String,
    pub name:       String,
    pub updated_at: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Theme {
    pub id:         String,
    pub name:       String,
    pub json:       String,
    pub updated_at: f64,
}

pub fn save_theme(con: &Connection, id: &str, name: &str, json: &str) -> Result<()> {
    con.execute(
        "INSERT OR REPLACE INTO catalog_themes (id, name, json, updated_at) VALUES (?1,?2,?3,?4)",
        params![id, name, json, now_secs()],
    )?;
    Ok(())
}

pub fn list_themes(con: &Connection) -> Result<Vec<ThemeSummary>> {
    let mut stmt = con.prepare(
        "SELECT id, name, updated_at FROM catalog_themes ORDER BY updated_at DESC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(ThemeSummary { id: r.get(0)?, name: r.get(1)?, updated_at: r.get(2)? })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

pub fn get_theme(con: &Connection, id: &str) -> Result<Option<Theme>> {
    let mut stmt = con.prepare(
        "SELECT id, name, json, updated_at FROM catalog_themes WHERE id = ?1",
    )?;
    let row = stmt.query_row(params![id], |r| {
        Ok(Theme { id: r.get(0)?, name: r.get(1)?, json: r.get(2)?, updated_at: r.get(3)? })
    });
    match row {
        Ok(t) => Ok(Some(t)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn delete_theme(con: &Connection, id: &str) -> Result<()> {
    con.execute("DELETE FROM catalog_themes WHERE id = ?1", params![id])?;
    Ok(())
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

// ── Tests (in-memory DB; exercise the whole data layer) ─────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    fn mem() -> Connection {
        let con = Connection::open_in_memory().unwrap();
        con.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        con.execute_batch(
            "CREATE TABLE files (id INTEGER PRIMARY KEY AUTOINCREMENT, path TEXT UNIQUE NOT NULL, hash TEXT NOT NULL, faiss_id INTEGER UNIQUE NOT NULL, mtime REAL NOT NULL DEFAULT 0);
             CREATE TABLE watched_folders (id INTEGER PRIMARY KEY AUTOINCREMENT, path TEXT UNIQUE NOT NULL, status TEXT NOT NULL DEFAULT 'watching', added_at REAL NOT NULL, last_event_at REAL NOT NULL DEFAULT 0);
             CREATE TABLE file_tags (id INTEGER PRIMARY KEY AUTOINCREMENT, file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE, category TEXT NOT NULL, value TEXT NOT NULL, source TEXT NOT NULL DEFAULT 'manual', created_at REAL NOT NULL DEFAULT 0, UNIQUE(file_id,category,value));
             CREATE TABLE catalog_themes (id TEXT PRIMARY KEY, name TEXT NOT NULL, json TEXT NOT NULL, updated_at REAL NOT NULL DEFAULT 0);",
        ).unwrap();
        con
    }
    fn seed_files(con: &Connection, paths: &[&str]) {
        for (i, p) in paths.iter().enumerate() {
            con.execute("INSERT INTO files (path,hash,faiss_id,mtime) VALUES (?1,?2,?3,0)",
                params![p, format!("h{i}"), i as i64]).unwrap();
        }
    }

    #[test]
    fn tags_set_replace_query_facets() {
        let con = mem();
        seed_files(&con, &["/lib/a.jpg", "/lib/b.jpg", "/lib/c.jpg"]);

        set_single_tag(&con, "/lib/a.jpg", "color", "grey", "auto").unwrap();
        set_single_tag(&con, "/lib/b.jpg", "color", "grey", "auto").unwrap();
        set_single_tag(&con, "/lib/a.jpg", "finish", "glossy", "manual").unwrap();
        add_tag(&con, "/lib/a.jpg", "custom", "bestseller", "manual").unwrap();

        // single-valued replace: a's color grey -> beige
        set_single_tag(&con, "/lib/a.jpg", "color", "beige", "auto").unwrap();
        let ta = tags_for_paths(&con, &["/lib/a.jpg".into()]).unwrap();
        assert!(ta.iter().any(|t| t.category == "color" && t.value == "beige"));
        assert!(!ta.iter().any(|t| t.category == "color" && t.value == "grey"));

        // facets: grey now only on b
        let f = tag_facets(&con).unwrap();
        assert!(f.iter().any(|x| x.category == "color" && x.value == "grey" && x.count == 1));

        // query: no filter = all
        assert_eq!(query_paths_by_tags(&con, &[]).unwrap().len(), 3);
        // single filter
        assert_eq!(query_paths_by_tags(&con, &[("color".into(), "grey".into())]).unwrap(),
                   vec!["/lib/b.jpg".to_string()]);
        // AND combo (the bug we fixed)
        assert_eq!(query_paths_by_tags(&con,
                     &[("color".into(), "beige".into()), ("finish".into(), "glossy".into())]).unwrap(),
                   vec!["/lib/a.jpg".to_string()]);
        // AND with no match
        assert!(query_paths_by_tags(&con,
                  &[("color".into(), "grey".into()), ("finish".into(), "glossy".into())]).unwrap().is_empty());

        // OR within the same category: color=beige OR color=grey -> both a and b
        let mut or_result = query_paths_by_tags(&con,
            &[("color".into(), "beige".into()), ("color".into(), "grey".into())]).unwrap();
        or_result.sort();
        assert_eq!(or_result, vec!["/lib/a.jpg".to_string(), "/lib/b.jpg".to_string()]);

        // OR within category combined with AND across category:
        // (color=beige OR color=grey) AND finish=glossy -> only a (b has no finish tag)
        assert_eq!(query_paths_by_tags(&con,
                     &[("color".into(), "beige".into()), ("color".into(), "grey".into()), ("finish".into(), "glossy".into())]).unwrap(),
                   vec!["/lib/a.jpg".to_string()]);

        remove_tag(&con, "/lib/a.jpg", "custom", "bestseller").unwrap();
        assert!(!tags_for_paths(&con, &["/lib/a.jpg".into()]).unwrap().iter().any(|t| t.value == "bestseller"));
    }

    /// Simulates a legacy customer DB (path-keyed file_tags, no file_id column)
    /// upgrading in place — the exact path an existing `.pictoria` takes.
    #[test]
    fn migrates_legacy_path_keyed_file_tags() {
        let con = Connection::open_in_memory().unwrap();
        con.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        con.execute_batch(
            "CREATE TABLE files (id INTEGER PRIMARY KEY AUTOINCREMENT, path TEXT UNIQUE NOT NULL, hash TEXT NOT NULL, faiss_id INTEGER UNIQUE NOT NULL, mtime REAL NOT NULL DEFAULT 0);
             CREATE TABLE file_tags (id INTEGER PRIMARY KEY AUTOINCREMENT, path TEXT NOT NULL, category TEXT NOT NULL, value TEXT NOT NULL, source TEXT NOT NULL DEFAULT 'manual', created_at REAL NOT NULL DEFAULT 0, UNIQUE(path,category,value));",
        ).unwrap();
        seed_files(&con, &["/lib/a.jpg", "/lib/b.jpg"]);
        con.execute("INSERT INTO file_tags (path,category,value,source,created_at) VALUES ('/lib/a.jpg','color','beige','auto',0)", []).unwrap();
        con.execute("INSERT INTO file_tags (path,category,value,source,created_at) VALUES ('/lib/a.jpg','finish','glossy','manual',0)", []).unwrap();
        // An orphan tag (path not in files) — must be dropped by the migration.
        con.execute("INSERT INTO file_tags (path,category,value,source,created_at) VALUES ('/gone/x.jpg','color','grey','auto',0)", []).unwrap();

        migrate_file_tags_to_file_id(&con).unwrap();

        // Valid tags survived, keyed by file_id now; orphan dropped.
        let ta = tags_for_paths(&con, &["/lib/a.jpg".into()]).unwrap();
        assert_eq!(ta.len(), 2);
        assert!(ta.iter().any(|t| t.category == "color" && t.value == "beige"));
        assert!(!tag_facets(&con).unwrap().iter().any(|f| f.value == "grey"));

        // The new ON DELETE CASCADE removes tags when the file row goes.
        delete_file(&con, "/lib/a.jpg").unwrap();
        assert!(tags_for_paths(&con, &["/lib/a.jpg".into()]).unwrap().is_empty());

        // Idempotent: a second run is a harmless no-op.
        migrate_file_tags_to_file_id(&con).unwrap();
    }

    #[test]
    fn folder_tree_structure() {
        let con = mem();
        seed_files(&con, &["/root/a.jpg", "/root/sub1/b.jpg", "/root/sub1/c.jpg", "/root/sub2/d.jpg"]);
        let tree = folder_tree(&con, "/root").unwrap();
        assert_eq!(tree.total, 4);
        assert_eq!(tree.direct, 1);
        let s1 = tree.children.iter().find(|c| c.name == "sub1").unwrap();
        assert_eq!(s1.total, 2);
        assert_eq!(s1.direct, 2);
        let missing = paths_missing_category_in_folder(&con, "/root", "color").unwrap();
        assert_eq!(missing.len(), 4);
    }

    #[test]
    fn catalog_theme_crud() {
        let con = mem();
        save_theme(&con, "t1", "My Catalog", "{\"pages\":[1]}").unwrap();
        save_theme(&con, "t2", "Second", "{}").unwrap();
        assert_eq!(list_themes(&con).unwrap().len(), 2);
        let g = get_theme(&con, "t1").unwrap().unwrap();
        assert_eq!(g.name, "My Catalog");
        assert_eq!(g.json, "{\"pages\":[1]}");
        save_theme(&con, "t1", "Renamed", "{\"x\":1}").unwrap(); // replace
        assert_eq!(get_theme(&con, "t1").unwrap().unwrap().name, "Renamed");
        assert_eq!(list_themes(&con).unwrap().len(), 2);
        delete_theme(&con, "t1").unwrap();
        assert!(get_theme(&con, "t1").unwrap().is_none());
        assert_eq!(list_themes(&con).unwrap().len(), 1);
    }

    #[test]
    fn file_find_move_delete() {
        let con = mem();
        insert_file(&con, "/x/a.jpg", "hh", 0, 1.0).unwrap();
        assert!(find_by_path(&con, "/x/a.jpg").unwrap().is_some());
        let (p, id) = find_by_hash(&con, "hh").unwrap();
        assert_eq!(p, Some("/x/a.jpg".to_string()));
        assert_eq!(id, Some(0));
        move_file(&con, "/x/a.jpg", "/x/b.jpg").unwrap();
        assert!(find_by_path(&con, "/x/a.jpg").unwrap().is_none());
        assert!(find_by_path(&con, "/x/b.jpg").unwrap().is_some());
        assert_eq!(next_vector_id(&con).unwrap(), 1);
    }

    #[test]
    fn watched_folder_lifecycle() {
        let con = mem();
        insert_watched_folder(&con, "/w").unwrap();
        seed_files(&con, &["/w/a.jpg", "/w/b.jpg"]);
        assert_eq!(folder_file_count(&con, "/w").unwrap(), 2);
        set_watched_folder_status(&con, "/w", "paused").unwrap();
        // paused folders excluded from active watch paths
        assert!(watched_folder_paths(&con).unwrap().is_empty());
        assert_eq!(list_watched_folders(&con).unwrap().len(), 1);
    }
}
