import sys
sys.dont_write_bytecode = True

import threading
import time

_lock  = threading.Lock()
_state = {
    "active": False, "total": 0, "done": 0, "current": "",
    "percent": 0, "phase": "idle", "errors": 0,
    "eta_sec": -1, "elapsed": 0, "file_types": {},
}

_phase_start_time:    float = 0.0
_done_at_phase_start: int   = 0


def set_progress(done=None, total=None, current=None, phase=None, errors=None):
    global _phase_start_time, _done_at_phase_start
    with _lock:
        if phase is not None and phase != _state["phase"]:
            _phase_start_time    = time.time()
            _done_at_phase_start = done if done is not None else _state["done"]

        if done    is not None: _state["done"]    = done
        if total   is not None: _state["total"]   = total
        if current is not None: _state["current"] = current
        if phase   is not None: _state["phase"]   = phase
        if errors  is not None: _state["errors"]  = errors

        if _state["total"] > 0:
            _state["percent"] = round(_state["done"] / _state["total"] * 100, 1)

        _state["active"]  = _state["phase"] != "idle"
        elapsed           = time.time() - _phase_start_time
        _state["elapsed"] = round(elapsed, 1)
        items_done        = _state["done"] - _done_at_phase_start
        items_remaining   = _state["total"] - _state["done"]

        if elapsed > 2 and items_done > 0 and items_remaining > 0:
            _state["eta_sec"] = round(items_remaining / (items_done / elapsed))
        else:
            _state["eta_sec"] = -1


def set_file_type_totals(counts: dict):
    with _lock:
        _state["file_types"] = {ext: {"done": 0, "total": count} for ext, count in counts.items()}


def increment_file_type(ext: str):
    with _lock:
        ext = ext.lower().lstrip(".")
        if ext in _state["file_types"]:
            _state["file_types"][ext]["done"] += 1
        else:
            _state["file_types"][ext] = {"done": 1, "total": 1}


def increment_errors():
    with _lock:
        _state["errors"] += 1


def get_progress() -> dict:
    with _lock:
        snap = dict(_state)
        snap["file_types"] = {k: dict(v) for k, v in _state["file_types"].items()}
        return snap


def reset():
    global _phase_start_time, _done_at_phase_start
    with _lock:
        _phase_start_time = 0.0
        _done_at_phase_start = 0
        _state.update({
            "active": False, "total": 0, "done": 0, "current": "",
            "percent": 0, "phase": "idle", "errors": 0,
            "eta_sec": -1, "elapsed": 0, "file_types": {},
        })
