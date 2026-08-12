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
import os
import queue
import threading

from flask import Flask, jsonify, request
from waitress import serve

import pipeline

LOG_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "logs")
os.makedirs(LOG_DIR, exist_ok=True)

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s  %(levelname)-7s %(message)s",
    handlers=[
        logging.StreamHandler(),
        logging.FileHandler(os.path.join(LOG_DIR, "sidecar.log"), encoding="utf-8"),
    ],
)
log = logging.getLogger("sidecar")

app = Flask(__name__)

_search_q: "queue.Queue[Job]" = queue.Queue()
_index_q: "queue.Queue[Job]" = queue.Queue()
_ready = threading.Event()


class Job:
    """One unit of work handed to the worker thread; the HTTP handler blocks
    on `event` until the worker fills in `result`."""

    __slots__ = ("kind", "payload", "priority", "event", "result")

    def __init__(self, kind: str, payload: dict, priority: str):
        self.kind = kind
        self.payload = payload
        self.priority = priority
        self.event = threading.Event()
        self.result: dict | None = None


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
    if job.kind == "describe":
        results = []
        for p in job.payload["paths"]:
            d = pipeline.describe(p)
            results.append({"path": p, "rose": d.rose, "gram": d.gram} if d
                            else {"path": p, "error": "decode-failed"})
            if job.priority != "search":
                _drain_search(query_cache)  # let a just-arrived search cut in, file by file
        return {"results": results}

    if job.kind == "verify":
        query_path = job.payload["query_path"]
        results = []
        for c in job.payload["candidate_paths"]:
            r = pipeline.verify(query_path, c, query_cache)
            r["path"] = c
            results.append(r)
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
