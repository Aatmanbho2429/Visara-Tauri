import os
import json
import sqlite3
import uuid
from datetime import datetime, timedelta
from app.config import DB_PATH, FAISS_DIR
from app.core.progress import set_progress, reset


# ═══════════════════════════════════════════════════════════════════════
# CONNECTION
# ═══════════════════════════════════════════════════════════════════════

def get_connection() -> sqlite3.Connection:
    os.makedirs(FAISS_DIR, exist_ok=True)
    con = sqlite3.connect(DB_PATH, check_same_thread=False)
    con.execute("PRAGMA journal_mode=WAL")
    con.execute("PRAGMA synchronous=NORMAL")

    # ── files table ───────────────────────────────────────────────────
    con.execute("""
        CREATE TABLE IF NOT EXISTS files (
            id       INTEGER PRIMARY KEY AUTOINCREMENT,
            path     TEXT    UNIQUE NOT NULL,
            hash     TEXT    NOT NULL,
            faiss_id INTEGER UNIQUE NOT NULL,
            mtime    REAL    NOT NULL DEFAULT 0
        )
    """)
    try:
        con.execute("ALTER TABLE files ADD COLUMN mtime REAL NOT NULL DEFAULT 0")
    except Exception:
        pass

    con.execute("CREATE INDEX IF NOT EXISTS idx_hash     ON files(hash)")
    con.execute("CREATE INDEX IF NOT EXISTS idx_path     ON files(path)")
    con.execute("CREATE INDEX IF NOT EXISTS idx_faiss_id ON files(faiss_id)")

    # ── activity_log table ────────────────────────────────────────────
    # One row per search or folder-load operation.
    # results / errors stored as JSON text columns.
    con.execute("""
        CREATE TABLE IF NOT EXISTS activity_log (
            id            TEXT    PRIMARY KEY,          -- UUID
            type          TEXT    NOT NULL,             -- 'search' | 'sync'
            timestamp     TEXT    NOT NULL,             -- UTC ISO-8601
            folder        TEXT    NOT NULL,
            query_image   TEXT,                         -- search only
            duration_sec  REAL    NOT NULL DEFAULT 0,
            result_count  INTEGER NOT NULL DEFAULT 0,   -- search only
            indexed_count INTEGER NOT NULL DEFAULT 0,   -- sync only
            error_count   INTEGER NOT NULL DEFAULT 0,
            results       TEXT    NOT NULL DEFAULT '[]',-- JSON
            errors        TEXT    NOT NULL DEFAULT '[]',-- JSON
            status        TEXT    NOT NULL              -- 'success'|'partial'|'error'
        )
    """)
    con.execute("""
        CREATE INDEX IF NOT EXISTS idx_al_timestamp ON activity_log(timestamp DESC)
    """)
    con.execute("""
        CREATE INDEX IF NOT EXISTS idx_al_type ON activity_log(type)
    """)

    con.commit()
    return con


# ═══════════════════════════════════════════════════════════════════════
# FILES TABLE
# ═══════════════════════════════════════════════════════════════════════

def cleanup_missing_in_folder(con: sqlite3.Connection, index, folder_path: str):
    folder_prefix = os.path.normpath(folder_path) + os.sep
    rows = con.execute(
        "SELECT path, faiss_id FROM files WHERE path LIKE ?",
        (folder_prefix + "%",)
    ).fetchall()

    missing_paths     = []
    missing_faiss_ids = []

    set_progress(done=0, total=len(rows), current="", phase="Database Cleanup", errors=0)

    for path, faiss_id in rows:
        if not os.path.exists(path):
            missing_paths.append((path,))
            missing_faiss_ids.append(faiss_id)

    if missing_paths:
        con.executemany("DELETE FROM files WHERE path=?", missing_paths)
        from app.core import indexer
        indexer.remove_embeddings(index, missing_faiss_ids)
        con.commit()
        set_progress(done=len(rows))
        reset()


def find_by_hash(con: sqlite3.Connection, hash_value: str):
    """Returns (path, faiss_id) or (None, None)"""
    row = con.execute(
        "SELECT path, faiss_id FROM files WHERE hash=?", (hash_value,)
    ).fetchone()
    return (row[0], row[1]) if row else (None, None)


def find_by_path(con: sqlite3.Connection, path: str):
    """Returns (faiss_id, hash, mtime) or None"""
    row = con.execute(
        "SELECT faiss_id, hash, mtime FROM files WHERE path=?", (path,)
    ).fetchone()
    return row if row else None


def get_next_faiss_id(con: sqlite3.Connection) -> int:
    row = con.execute("SELECT MAX(faiss_id) FROM files").fetchone()
    return (row[0] + 1) if row[0] is not None else 0


def insert_file(con: sqlite3.Connection, path: str, hash_value: str,
                faiss_id: int, mtime: float):
    con.execute(
        "INSERT OR REPLACE INTO files (path, hash, faiss_id, mtime) VALUES (?,?,?,?)",
        (path, hash_value, faiss_id, mtime)
    )


def move_file(con: sqlite3.Connection, old_path: str, new_path: str):
    con.execute("UPDATE files SET path=? WHERE path=?", (new_path, old_path))


def delete_file(con: sqlite3.Connection, path: str):
    con.execute("DELETE FROM files WHERE path=?", (path,))


def get_folder_id_map(con: sqlite3.Connection, folder_path: str) -> dict:
    folder_prefix = os.path.normpath(folder_path) + os.sep
    rows = con.execute(
        "SELECT faiss_id, path FROM files WHERE path LIKE ?",
        (folder_prefix + "%",)
    ).fetchall()
    return {r[0]: r[1] for r in rows}


def get_folder_hashes(con: sqlite3.Connection, folder_path: str) -> set:
    folder_prefix = os.path.normpath(folder_path) + os.sep
    rows = con.execute(
        "SELECT hash FROM files WHERE path LIKE ?",
        (folder_prefix + "%",)
    ).fetchall()
    return {r[0] for r in rows}


def get_files_by_hashes(con: sqlite3.Connection, hashes: set) -> list:
    if not hashes:
        return []
    placeholders = ",".join("?" * len(hashes))
    return con.execute(
        f"SELECT path, faiss_id FROM files WHERE hash IN ({placeholders})",
        list(hashes)
    ).fetchall()


def get_folder_file_count(con: sqlite3.Connection, folder_path: str) -> int:
    folder_prefix = os.path.normpath(folder_path) + os.sep
    cur = con.execute(
        "SELECT COUNT(*) FROM files WHERE path LIKE ?",
        (folder_prefix + "%",)
    )
    return cur.fetchone()[0]


def get_all_path(con: sqlite3.Connection) -> list:
    return con.execute("SELECT path FROM files").fetchall()


# ═══════════════════════════════════════════════════════════════════════
# ACTIVITY LOG TABLE — write
# ═══════════════════════════════════════════════════════════════════════

RETAIN_DAYS = 30


def _prune_activity_log(con: sqlite3.Connection) -> None:
    """Delete entries older than RETAIN_DAYS."""
    cutoff = (datetime.utcnow() - timedelta(days=RETAIN_DAYS)).isoformat()
    con.execute("DELETE FROM activity_log WHERE timestamp < ?", (cutoff,))


def log_search(con: sqlite3.Connection, query_image: str, folder: str,
               results: list, errors: list, duration_sec: float) -> None:
    """Insert one search entry and prune old rows."""
    status = ("error"   if not results and errors else
              "partial" if errors          else "success")
    _prune_activity_log(con)
    con.execute("""
        INSERT INTO activity_log
            (id, type, timestamp, folder, query_image, duration_sec,
             result_count, indexed_count, error_count, results, errors, status)
        VALUES (?,?,?,?,?,?,?,?,?,?,?,?)
    """, (
        str(uuid.uuid4()),
        "search",
        datetime.utcnow().isoformat(),
        folder,
        query_image,
        round(duration_sec, 2),
        len(results),   # result_count
        0,              # indexed_count (n/a for search)
        len(errors),
        json.dumps(results, ensure_ascii=False),
        json.dumps(errors,  ensure_ascii=False),
        status,
    ))
    con.commit()


def log_sync(con: sqlite3.Connection, folder: str, indexed_count: int,
             errors: list, duration_sec: float) -> None:
    """Insert one sync entry and prune old rows."""
    status = ("error"   if indexed_count == 0 and errors else
              "partial" if errors                        else "success")
    _prune_activity_log(con)
    con.execute("""
        INSERT INTO activity_log
            (id, type, timestamp, folder, query_image, duration_sec,
             result_count, indexed_count, error_count, results, errors, status)
        VALUES (?,?,?,?,?,?,?,?,?,?,?,?)
    """, (
        str(uuid.uuid4()),
        "sync",
        datetime.utcnow().isoformat(),
        folder,
        None,           # query_image (n/a for sync)
        round(duration_sec, 2),
        0,              # result_count (n/a for sync)
        indexed_count,
        len(errors),
        "[]",           # results (n/a for sync)
        json.dumps(errors, ensure_ascii=False),
        status,
    ))
    con.commit()


# ═══════════════════════════════════════════════════════════════════════
# ACTIVITY LOG TABLE — read
# ═══════════════════════════════════════════════════════════════════════

def get_activity_log(con: sqlite3.Connection, limit: int = 300) -> list:
    """
    Returns up to `limit` entries ordered newest-first.
    JSON columns (results, errors) are parsed back to lists.
    """
    rows = con.execute("""
        SELECT id, type, timestamp, folder, query_image, duration_sec,
               result_count, indexed_count, error_count, results, errors, status
        FROM   activity_log
        ORDER  BY timestamp DESC
        LIMIT  ?
    """, (limit,)).fetchall()

    entries = []
    for row in rows:
        entries.append({
            "id":            row[0],
            "type":          row[1],
            "timestamp":     row[2],
            "folder":        row[3],
            "query_image":   row[4],
            "duration_sec":  row[5],
            "result_count":  row[6],
            "indexed_count": row[7],
            "error_count":   row[8],
            "results":       json.loads(row[9]  or "[]"),
            "errors":        json.loads(row[10] or "[]"),
            "status":        row[11],
        })
    return entries
