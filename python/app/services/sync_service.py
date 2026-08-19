import os
import time
from collections import Counter
from concurrent.futures import ThreadPoolExecutor

from app.config import BATCH_SIZE, NUM_WORKERS
from app.core import database as db
from app.core import indexer
from app.core.progress import (set_progress, increment_errors,
                               increment_file_type, set_file_type_totals, reset)
from app.core.embedder import Embedder
from app.utils.file_utils import fast_hash, scan_images
from app.utils.image_loader import preprocess_batch_parallel


def _is_under(path: str, folder: str) -> bool:
    return os.path.normpath(path).startswith(os.path.normpath(folder) + os.sep)


def sync_folder(index, folder_path: str, response) -> None:
    folder_path     = os.path.normpath(folder_path)
    current_files   = list(scan_images(folder_path))
    seen_hashes     = set()
    needs_embedding = []
    t_start         = time.time()

    con = db.get_connection()
    db.cleanup_missing_in_folder(con, index, folder_path)

    # ── Step 1: Hash ──────────────────────────────────────────────────────
    set_progress(done=0, total=len(current_files), current="", phase="hashing", errors=0)

    def hash_one(path):
        thread_con = db.get_connection()
        try:
            current_mtime = os.path.getmtime(path)
            existing      = db.find_by_path(thread_con, path)
            if existing and abs(existing[2] - current_mtime) < 0.001:
                return path, existing[1], None, current_mtime
            return path, fast_hash(path), None, current_mtime
        except Exception as e:
            return path, None, str(e), 0.0
        finally:
            thread_con.close()

    with ThreadPoolExecutor(max_workers=NUM_WORKERS) as ex:
        hash_results = list(ex.map(hash_one, current_files))

    hashed_done = 0
    for path, h, err, mtime in hash_results:
        hashed_done += 1
        set_progress(done=hashed_done, current=os.path.basename(path))

        if err:
            response.data["errors"].append({"file": path, "reason": "error hashing file: " + err})
            continue

        seen_hashes.add(h)
        existing_path, existing_faiss_id = db.find_by_hash(con, h)

        if existing_faiss_id is not None:
            if existing_path == path:
                pass
            elif _is_under(existing_path, folder_path):
                needs_embedding.append((path, h, mtime))
            elif not os.path.exists(existing_path):
                db.move_file(con, existing_path, path)
                con.commit()
            else:
                needs_embedding.append((path, h, mtime))
        else:
            existing_by_path = db.find_by_path(con, path)
            if existing_by_path:
                indexer.remove_embeddings(index, [existing_by_path[0]])
                db.delete_file(con, path)
                con.commit()
            needs_embedding.append((path, h, mtime))

    # ── Step 2: Embed ─────────────────────────────────────────────────────
    total    = len(needs_embedding)
    done     = 0
    embedder = Embedder()

    ext_counts = Counter(
        os.path.splitext(p)[1].lower().lstrip(".") for p, _, _ in needs_embedding
    )
    reset()
    set_file_type_totals(dict(ext_counts))
    set_progress(done=0, total=total if total > 0 else 1, phase="embedding")

    for i in range(0, total, BATCH_SIZE):
        chunk        = needs_embedding[i:i + BATCH_SIZE]
        batch_paths  = [p        for p, _, _     in chunk]
        hash_lookup  = {p: h     for p, h, _     in chunk}
        mtime_lookup = {p: mtime for p, _, mtime in chunk}

        set_progress(current=batch_paths[0])

        batch, valid_paths, failed = preprocess_batch_parallel(batch_paths)

        for f in failed:
            increment_errors()
            response.data["errors"].append(f)

        if batch is not None:
            embs = embedder.embed_batch(batch)
            for path, emb in zip(valid_paths, embs):
                faiss_id = db.get_next_faiss_id(con)
                indexer.add_embedding(index, emb, faiss_id)
                db.insert_file(con, path, hash_lookup[path], faiss_id, mtime_lookup[path])
                response.data["success"].append({"file": path, "reason": "Passed embedding"})
                increment_file_type(os.path.splitext(path)[1])
            con.commit()

        done += len(chunk)
        set_progress(done=done)

    # ── Step 3: Remove deleted ────────────────────────────────────────────
    all_hashes     = db.get_folder_hashes(con, folder_path)
    deleted_hashes = all_hashes - seen_hashes

    if deleted_hashes:
        rows             = db.get_files_by_hashes(con, deleted_hashes)
        faiss_ids_to_del = []
        for path, faiss_id in rows:
            faiss_ids_to_del.append(faiss_id)
            db.delete_file(con, path)
        indexer.remove_embeddings(index, faiss_ids_to_del)
        con.commit()

    # ── Write activity log ─────────────────────────────────────────────────
    try:
        db.log_sync(
            con           = con,
            folder        = folder_path,
            indexed_count = len(response.data["success"]),
            errors        = response.data["errors"],
            duration_sec  = time.time() - t_start,
        )
    except Exception as e:
        print(f"[activity_log] write failed: {e}", flush=True)

    con.close()
    reset()
