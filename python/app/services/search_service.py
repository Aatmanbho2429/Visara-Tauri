import os
import time

from app.config import SEARCH_ORIENTATIONS, SEARCH_OVERSAMPLE
from app.core import database as db
from app.core import indexer
from app.core.embedder import Embedder
from app.core.progress import set_progress, reset
from app.services.sync_service import sync_folder
from app.utils.file_utils import scan_images
from app.utils.image_loader import preprocess_batch_parallel
from app.utils.orientation import (ORIENTATION_LABELS, build_orientation_batch,
                                   resolve_orientations)


class BaseResponse:
    def __init__(self):
        self.status  = True
        self.message = ""
        self.code    = 200
        self.data    = {"success": [], "errors": [], "results": []}

    def to_dict(self) -> dict:
        """Envelope the API and the Tauri layer expect: {success, message, data}."""
        return {
            "success": self.status,
            "message": self.message,
            "data":    self.data if self.status else None,
        }


def _get_query_embeddings(query_image: str, response: BaseResponse) -> tuple:
    """
    Embed the query image in every orientation we search over.

    Returns (embeddings, names) where embeddings is (N, EMB_DIM) L2-normalized
    and names[i] is the orientation that produced row i. The image is decoded
    and preprocessed once; the orientations are derived from that tensor.
    """
    embedder = Embedder()
    batch, valid, failed = preprocess_batch_parallel([query_image])
    if not valid:
        response.message = "Failed to process query image."
        raise RuntimeError(failed[0]["reason"])

    orientations    = resolve_orientations(SEARCH_ORIENTATIONS)
    variants, names = build_orientation_batch(batch[0], orientations)
    return embedder.embed_batch(variants), names


def _merge_orientation_hits(scores, ids, names, id_map: dict) -> list:
    """
    Collapse the per-orientation result rows into one ranked list.

    A file that is matched by several orientations keeps only its best score,
    tagged with the orientation that achieved it — so a mirrored copy of an
    indexed tile is reported once, at the similarity it actually earned, rather
    than eight times.
    """
    best = {}

    for row, orientation in enumerate(names):
        for idx, score in zip(ids[row], scores[row]):
            idx = int(idx)
            if idx == -1 or idx not in id_map:
                continue
            score = float(score)
            current = best.get(idx)
            if current is None or score > current[0]:
                best[idx] = (score, orientation)

    ranked = sorted(best.items(), key=lambda kv: kv[1][0], reverse=True)

    results = []
    for rank, (idx, (score, orientation)) in enumerate(ranked):
        results.append({
            "rank":              rank + 1,
            "path":              id_map[idx],
            "similarity":        score,
            "orientation":       orientation,
            "orientation_label": ORIENTATION_LABELS.get(orientation, orientation),
        })
    return results


def search(query_image: str, folder_path: str, top_k: int) -> dict:
    response    = BaseResponse()
    folder_path = os.path.normpath(folder_path)
    index       = indexer.load_index()
    t_start     = time.time()

    if not os.path.exists(query_image):
        response.status  = False
        response.message = "Query image not found."
        response.code    = 400
        return response.to_dict()

    if not os.path.isdir(folder_path):
        response.status  = False
        response.message = "Folder not found."
        response.code    = 400
        return response.to_dict()

    if index == "Error loading faiss.index":
        response.status  = False
        response.message = "Failed to load index. Please sync your folders first."
        response.code    = 500
        return response.to_dict()

    # ── Only sync if disk count != DB count ──────────────────────────────
    con        = db.get_connection()
    db_count   = db.get_folder_file_count(con, folder_path)
    disk_count = sum(1 for _ in scan_images(folder_path))
    con.close()

    if db_count != disk_count:
        sync_folder(index, folder_path, response)
        indexer.save_index(index)

    # ── Similarity search ─────────────────────────────────────────────────
    set_progress(phase="searching", done=0, total=1,
                 current=os.path.basename(query_image))

    con = db.get_connection()
    try:
        id_map         = db.get_folder_id_map(con, folder_path)
        queries, names = _get_query_embeddings(query_image, response)

        # The index spans every synced folder, so results are filtered down to
        # the selected one afterwards. Search deeper than top_k to keep that
        # filter from starving the merged list.
        depth           = max(top_k * SEARCH_OVERSAMPLE, top_k)
        scores, indices = indexer.search_index_multi(index, queries, depth)

        response.data["results"] = _merge_orientation_hits(
            scores, indices, names, id_map
        )[:top_k]
        response.data["orientations_searched"] = list(names)
    except Exception as e:
        response.status = True
        response.data["errors"].append({"file": query_image, "reason": str(e)})
    finally:
        # ── Write activity log ──────────────────────────────────────────
        try:
            db.log_search(
                con          = con,
                query_image  = query_image,
                folder       = folder_path,
                results      = response.data["results"],
                errors       = response.data["errors"],
                duration_sec = time.time() - t_start,
            )
        except Exception as e:
            print(f"[activity_log] write failed: {e}", flush=True)
        con.close()

    reset()

    if response.message == "":
        response.message = "Search completed successfully"
    response.code = 200
    return response.to_dict()
