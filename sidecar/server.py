"""HTTP sidecar for Pictoria's design-match pipeline.

Endpoints: GET /health, POST /describe, POST /verify.

One background worker thread does all the heavy work, one job at a time —
never two Gabor/Gram/SIFT computations running concurrently, which avoids
both CPU oversubscription and calling the shared model from two threads at
once. Two priority lanes feed it: "search" jobs (interactive, user is
waiting) always get pulled before "index" jobs (background watcher work),
so a search never waits for more than the single file currently in flight —
indexing effectively pauses and resumes around it.
"""
from __future__ import annotations

import logging
import logging.handlers
import os
import queue
import sys
import threading
import time

from flask import Flask, jsonify, request
from waitress import serve

import pipeline


def _base_dir() -> str:
    """Directory to anchor `logs/` in.

    `__file__` is only meaningful for the plain-script case (`python
    server.py`, used in dev). Under PyInstaller's onefile bootloader,
    `__file__` resolves *inside* the `_MEI*` temp extraction dir, which is
    wiped when the process exits — a log written there vanishes with it.
    `sys.executable` is the actual frozen `.exe` path in that case.
    """
    if getattr(sys, "frozen", False):
        return os.path.dirname(sys.executable)
    return os.path.dirname(os.path.abspath(__file__))


def _log_dir() -> str:
    """Prefer `logs/` next to the binary (matches dev, keeps everything in
    one place). Falls back to a per-user directory if that's not writable —
    e.g. a `perMachine` install placing the exe under Program Files."""
    primary = os.path.join(_base_dir(), "logs")
    try:
        os.makedirs(primary, exist_ok=True)
        probe = os.path.join(primary, ".write_test")
        with open(probe, "w") as f:
            f.write("")
        os.remove(probe)
        return primary
    except OSError:
        fallback = os.path.join(os.environ.get("LOCALAPPDATA", os.path.expanduser("~")), "pictoria-sidecar", "logs")
        os.makedirs(fallback, exist_ok=True)
        return fallback


LOG_DIR = _log_dir()

logging.basicConfig(
    # DEBUG so the per-file/per-candidate [timing] breakdown in pipeline.py
    # (decode/gabor/gram, decode/sift/match/ransac) actually gets written —
    # at INFO those lines are silently dropped and only the per-job totals
    # in _run() below show up.
    level=logging.DEBUG,
    format="%(asctime)s  %(levelname)-7s %(message)s",
    handlers=[
        logging.StreamHandler(),
        # Rotating, not plain FileHandler: DEBUG-level per-file logging over
        # a large folder index adds up fast, and this file has no other
        # cap on it (unlike the Rust side's tauri-plugin-log KeepOne).
        logging.handlers.RotatingFileHandler(
            os.path.join(LOG_DIR, "sidecar.log"), maxBytes=10_000_000, backupCount=2, encoding="utf-8",
        ),
    ],
)
log = logging.getLogger("sidecar")
log.info("log directory: %s", LOG_DIR)

app = Flask(__name__)

_search_q: "queue.Queue[Job]" = queue.Queue()
_index_q: "queue.Queue[Job]" = queue.Queue()
_ready = threading.Event()


class Job:
    """One unit of work handed to the worker thread; the HTTP handler blocks
    on `event` until the worker fills in `result`."""

    __slots__ = ("kind", "payload", "priority", "event", "result", "t_created")

    def __init__(self, kind: str, payload: dict, priority: str):
        self.kind = kind
        self.payload = payload
        self.priority = priority
        self.event = threading.Event()
        self.result: dict | None = None
        self.t_created = time.perf_counter()  # for queue_wait_ms below


def _submit(kind: str, payload: dict, priority: str) -> dict:
    job = Job(kind, payload, priority)
    (_search_q if priority == "search" else _index_q).put(job)
    job.event.wait()
    return job.result


def _worker() -> None:
    """Loads the model once, then serves jobs forever — search lane first,
    always, so it can only ever be blocked by one already-running index job."""
    pipeline.load_model()
    _ready.set()
    log.info("model loaded, sidecar ready")

    query_cache: dict = {}
    while True:
        try:
            job = _search_q.get_nowait()
        except queue.Empty:
            try:
                job = _index_q.get(timeout=0.2)
            except queue.Empty:
                continue
        try:
            job.result = _run(job, query_cache)
        except Exception as e:  # noqa: BLE001
            log.exception("job failed: %s", job.kind)
            job.result = {"error": str(e)}
        job.event.set()


def _drain_search(query_cache: dict) -> None:
    """Run every search job waiting right now, to completion, before the
    caller (an index job) is allowed to touch its next file. This is what
    makes indexing 'pause' the instant a search arrives and 'resume' the
    moment the search is done, rather than only between whole batches."""
    while True:
        try:
            job = _search_q.get_nowait()
        except queue.Empty:
            return
        try:
            job.result = _run(job, query_cache)
        except Exception as e:  # noqa: BLE001
            log.exception("job failed: %s", job.kind)
            job.result = {"error": str(e)}
        job.event.set()


def _run(job: Job, query_cache: dict) -> dict:
    # `queue_wait_ms` is how long the job sat behind other work (mostly
    # relevant for "index" priority jobs, which yield to a just-arrived
    # search — see `_drain_search` below); `compute_ms` is the pipeline
    # work itself. Split out so a slow search can be told apart from a
    # search that was merely waiting behind a big indexing chunk.
    queue_wait_ms = (time.perf_counter() - job.t_created) * 1000
    t_start = time.perf_counter()

    if job.kind == "describe":
        results = []
        for p in job.payload["paths"]:
            d = pipeline.describe(p)
            results.append({"path": p, "rose": d.rose, "gram": d.gram, "color": d.color,
                            "dominant": d.dominant} if d
                            else {"path": p, "error": "decode-failed"})
            if job.priority != "search":
                _drain_search(query_cache)  # let a just-arrived search cut in, file by file
        compute_ms = (time.perf_counter() - t_start) * 1000
        log.info(
            "[timing] job kind=describe priority=%s n=%d queue_wait_ms=%.2f compute_ms=%.2f total_ms=%.2f",
            job.priority, len(job.payload["paths"]), queue_wait_ms, compute_ms, queue_wait_ms + compute_ms,
        )
        return {"results": results}

    if job.kind == "verify":
        query_path = job.payload["query_path"]
        results = []
        for c in job.payload["candidate_paths"]:
            r = pipeline.verify(query_path, c, query_cache)
            r["path"] = c
            results.append(r)
        compute_ms = (time.perf_counter() - t_start) * 1000
        log.info(
            "[timing] job kind=verify priority=%s n=%d queue_wait_ms=%.2f compute_ms=%.2f total_ms=%.2f",
            job.priority, len(job.payload["candidate_paths"]), queue_wait_ms, compute_ms, queue_wait_ms + compute_ms,
        )
        return {"results": results}

    return {"error": f"unknown job kind: {job.kind}"}


@app.get("/health")
def health():
    """Readiness probe — the Rust side shows 'Model is loading...' until this is true."""
    return jsonify({"ready": _ready.is_set()})


@app.post("/describe")
def describe_route():
    """Body: {paths: [str], priority?: 'search'|'index'}. One entry per path in `results`."""
    body = request.get_json(force=True) or {}
    paths = body.get("paths")
    if not paths:
        return jsonify({"error": "paths is required"}), 400
    return jsonify(_submit("describe", {"paths": paths}, body.get("priority", "index")))


@app.post("/verify")
def verify_route():
    """Body: {query_path: str, candidate_paths: [str], priority?: 'search'|'index'}."""
    body = request.get_json(force=True) or {}
    query_path = body.get("query_path")
    candidates = body.get("candidate_paths")
    if not query_path or not candidates:
        return jsonify({"error": "query_path and candidate_paths are required"}), 400
    payload = {"query_path": query_path, "candidate_paths": candidates}
    return jsonify(_submit("verify", payload, body.get("priority", "search")))


def main() -> None:
    threading.Thread(target=_worker, daemon=True).start()
    port = int(os.environ.get("SIDECAR_PORT", "8756"))
    log.info("sidecar listening on 127.0.0.1:%d", port)
    serve(app, host="127.0.0.1", port=port, threads=8)


if __name__ == "__main__":
    main()
