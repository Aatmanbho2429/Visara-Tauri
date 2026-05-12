import sys
sys.dont_write_bytecode = True

import threading
from fastapi import APIRouter
from app.api.v1.search.schemas import SearchRequest
from app.core.progress import get_progress, reset

router = APIRouter(prefix="/search", tags=["search"])

_lock  = threading.Lock()
_state = {
    "running":      False,
    "done":         False,
    "error":        None,
    "results":      [],
    "failed_files": [],
}


def _run_search(image_path: str, folder_path: str, top_k: int):
    global _state
    try:
        from app.services.search_service import execute_search
        results, failed = execute_search(image_path, folder_path, top_k)
        with _lock:
            _state.update({
                "running": False, "done": True, "error": None,
                "results": results, "failed_files": failed,
            })
    except Exception as e:
        reset()
        with _lock:
            _state.update({
                "running": False, "done": True, "error": str(e),
                "results": [], "failed_files": [],
            })


@router.post("/start")
async def start_search(req: SearchRequest):
    global _state
    with _lock:
        if _state["running"]:
            return {"success": False, "message": "A search is already in progress.", "data": None}
        _state = {"running": True, "done": False, "error": None, "results": [], "failed_files": []}

    threading.Thread(
        target=_run_search,
        args=(req.image_path, req.folder_path, req.top_k),
        daemon=True
    ).start()

    return {"success": True, "message": "Search started", "data": None}


@router.get("/progress")
async def get_search_progress():
    with _lock:
        state = dict(_state)

    progress = get_progress()

    # Fatal error — search failed completely
    if state["done"] and state["error"]:
        return {
            "success": False,
            "message": state["error"],
            "data":    None
        }

    # Still running — return progress snapshot
    if not state["done"]:
        return {
            "success": True,
            "message": "",
            "data": {
                "done":     False,
                "progress": progress,
            }
        }

    # Done successfully (may include per-file failures)
    return {
        "success": True,
        "message": "Search complete",
        "data": {
            "done":         True,
            "results":      state["results"],
            "failed_files": state["failed_files"],
        }
    }
