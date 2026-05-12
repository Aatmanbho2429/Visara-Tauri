import sys
sys.dont_write_bytecode = True

import os
from app.core.embedder import Embedder
from app.core import indexer, database as db
from app.core.progress import set_progress, reset
from app.utils.image_loader import preprocess_batch_parallel
from app.utils.file_utils import scan_images
from app.services.sync_service import sync_folder


def execute_search(image_path: str, folder_path: str, top_k: int) -> tuple:
    """
    Returns (results, failed_files).
    Raises RuntimeError on fatal errors that stop the entire process.
    Per-file errors are collected into failed_files and don't stop the search.
    """
    folder_path  = os.path.normpath(folder_path)
    failed_files = []

    # ── Fatal checks ──────────────────────────────────────────────────
    embedder = Embedder()
    if not embedder.is_ready:
        raise RuntimeError("AI model not loaded. Please restart the application and login again.")

    if not os.path.exists(image_path):
        raise RuntimeError("The selected reference image no longer exists.")

    if not os.path.isdir(folder_path):
        raise RuntimeError("The selected folder no longer exists.")

    # ── Load FAISS index ──────────────────────────────────────────────
    try:
        index = indexer.load_index()
    except RuntimeError as e:
        raise RuntimeError(str(e))

    # ── Sync folder if needed ─────────────────────────────────────────
    con        = db.get_connection()
    db_count   = db.get_folder_file_count(con, folder_path)
    con.close()
    disk_count = sum(1 for _ in scan_images(folder_path))

    if db_count != disk_count:
        sync_errors = sync_folder(index, folder_path)
        failed_files.extend(sync_errors)
        indexer.save_index(index)

    # ── Embed query image ─────────────────────────────────────────────
    set_progress(phase="Searching", done=0, total=1, current=os.path.basename(image_path))

    try:
        batch, valid, failed = preprocess_batch_parallel([image_path])
        if not valid:
            reason = failed[0]["reason"] if failed else "Unknown error"
            raise RuntimeError(f"Could not process the reference image: {reason}")
        query_emb = embedder.embed_batch(batch)[0]
    except RuntimeError:
        raise
    except Exception as e:
        raise RuntimeError(f"Failed to analyse the reference image: {e}")

    # ── FAISS search ──────────────────────────────────────────────────
    con = db.get_connection()
    try:
        id_map         = db.get_folder_id_map(con, folder_path)
        scores, indices = indexer.search_index(index, query_emb, top_k)

        results = []
        for rank, (idx, score) in enumerate(zip(indices, scores)):
            if idx == -1:
                continue
            if idx in id_map:
                path = id_map[idx]
                results.append({
                    "rank":       rank + 1,
                    "path":       path,
                    "name":       os.path.basename(path),
                    "similarity": round(float(score) * 100, 1),
                })
    finally:
        con.close()

    reset()
    return results, failed_files
