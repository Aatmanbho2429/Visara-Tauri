"""HTTP sidecar for Pictoria's design-match pipeline.

Endpoints: GET /health, POST /model, POST /model/reset, POST /describe,
POST /verify.

Two models are involved and they load at different times. The bundled
mobilenet (gram) loads on the worker thread at startup and gates `ready`. The
encrypted DINO model (embeddings) can only load once Rust posts the licence
key to /model after a successful token-validate, and gates `embed_ready` —
which is what indexing and search actually wait on, since a descriptor
without its embedding is useless to the near-family scan.

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
from concurrent.futures import ThreadPoolExecutor

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

# SIFT/RANSAC (pipeline.verify_one) are OpenCV C++ calls that release the
# GIL, so verifying a shortlist's candidates concurrently scales close to
# linearly with cores instead of paying ~350-450ms per candidate serially —
# see the `verify` branch of `_run()` below and `pipeline.verify_one`'s
# docstring for the timeout this used to blow through.
_VERIFY_WORKERS = max(1, min(8, os.cpu_count() or 4))

app = Flask(__name__)

_search_q: "queue.Queue[Job]" = queue.Queue()
_index_q: "queue.Queue[Job]" = queue.Queue()
_ready = threading.Event()

# Set when the one-time model load fails. The worker thread dies at that point
# and `_ready` can never be set, so without recording *why*, the sidecar looks
# identical to one that is merely slow: the HTTP server keeps answering and
# /health keeps reporting not-ready, forever. Surfaced through /health so the
# app can show a real error instead of an eternal "Model is loading...".
_load_error: "str | None" = None


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
    global _load_error
    try:
        pipeline.load_model()
    except Exception as e:  # noqa: BLE001 - any failure here is terminal
        # Nothing this thread can do to recover, but it must not die silently:
        # every job would then block on `job.event` forever with no explanation.
        _load_error = f"{type(e).__name__}: {e}"
        log.exception("model load FAILED - sidecar cannot become ready")
        return

    _ready.set()
    log.info("model loaded, sidecar ready")

    while True:
        try:
            job = _search_q.get_nowait()
        except queue.Empty:
            try:
                job = _index_q.get(timeout=0.2)
            except queue.Empty:
                continue
        try:
            job.result = _run(job)
        except Exception as e:  # noqa: BLE001
            log.exception("job failed: %s", job.kind)
            job.result = {"error": str(e)}
        job.event.set()


def _drain_search() -> None:
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
            job.result = _run(job)
        except Exception as e:  # noqa: BLE001
            log.exception("job failed: %s", job.kind)
            job.result = {"error": str(e)}
        job.event.set()


def _run(job: Job) -> dict:
    # `queue_wait_ms` is how long the job sat behind other work (mostly
    # relevant for "index" priority jobs, which yield to a just-arrived
    # search — see `_drain_search` below); `compute_ms` is the pipeline
    # work itself. Split out so a slow search can be told apart from a
    # search that was merely waiting behind a big indexing chunk.
    queue_wait_ms = (time.perf_counter() - job.t_created) * 1000
    t_start = time.perf_counter()

    if job.kind == "describe":
        # Checked once for the whole batch rather than per file: without the
        # DINO model every single describe would fail identically, and Rust
        # gates on `embed_ready` precisely so this can't happen — so report it
        # as one job-level error instead of N indistinguishable per-file ones.
        if not pipeline.embed_model_ready():
            log.error("describe rejected: DINO embed model not loaded")
            return {"error": "embed-model-not-loaded"}
        results = []
        for p in job.payload["paths"]:
            d = pipeline.describe(p)
            results.append({"path": p, "embed": d.embed, "rose": d.rose, "gram": d.gram,
                            "color": d.color, "dominant": d.dominant} if d
                            else {"path": p, "error": "decode-failed"})
            if job.priority != "search":
                _drain_search()  # let a just-arrived search cut in, file by file
        compute_ms = (time.perf_counter() - t_start) * 1000
        log.info(
            "[timing] job kind=describe priority=%s n=%d queue_wait_ms=%.2f compute_ms=%.2f total_ms=%.2f",
            job.priority, len(job.payload["paths"]), queue_wait_ms, compute_ms, queue_wait_ms + compute_ms,
        )
        return {"results": results}

    if job.kind == "verify":
        query_path = job.payload["query_path"]
        candidates = job.payload["candidate_paths"]
        mirror = bool(job.payload.get("mirror", False))
        # once, sequential — see prepare_query's docstring
        query_state = pipeline.prepare_query(query_path, mirror=mirror)

        def _verify(c: str) -> dict:
            r = pipeline.verify_one(query_state, c)
            r["path"] = c
            return r

        with ThreadPoolExecutor(max_workers=_VERIFY_WORKERS) as pool:
            results = list(pool.map(_verify, candidates))
        compute_ms = (time.perf_counter() - t_start) * 1000
        log.info(
            "[timing] job kind=verify priority=%s n=%d mirror=%s queue_wait_ms=%.2f compute_ms=%.2f total_ms=%.2f",
            job.priority, len(job.payload["candidate_paths"]), mirror,
            queue_wait_ms, compute_ms, queue_wait_ms + compute_ms,
        )
        return {"results": results}

    return {"error": f"unknown job kind: {job.kind}"}


@app.get("/health")
def health():
    """Readiness probe — the Rust side shows 'Model is loading...' until this is true.

    `ready` covers the bundled mobilenet only; `embed_ready` reports the
    licence-gated DINO model, which stays false (legitimately, not as an error)
    until Rust posts a key to /model. `embed_dim` lets Rust confirm the model
    matches the vector store's entry stride before it indexes anything.

    `error` is non-null only when the startup model load failed outright, which
    is a terminal state: it will never become ready, so the app should say so
    rather than keep waiting."""
    return jsonify({
        "ready": _ready.is_set(),
        "error": _load_error,
        "embed_ready": pipeline.embed_model_ready(),
        "embed_dim": pipeline.embed_model_dim(),
    })


@app.post("/model")
def model_route():
    """Body: {key: str}. Decrypt and compile the DINO model. Idempotent.

    Blocks until the model is usable (decrypt + ORT compile is seconds, not
    milliseconds) and answers with the measured embedding width, so Rust learns
    the outcome from this one call and never has to poll for it.

    The key is held only for the life of this process and is never logged or
    written to disk — the exception text is deliberately generic for the same
    reason.
    """
    body = request.get_json(force=True) or {}
    key = body.get("key")
    if not key:
        return jsonify({"error": "key is required"}), 400
    try:
        dim = pipeline.load_embed_model(key)
    except Exception as e:  # noqa: BLE001 - reported to Rust, not swallowed
        log.exception("DINO embed model load FAILED")
        return jsonify({"error": f"{type(e).__name__}: {e}"}), 500
    return jsonify({"ok": True, "embed_dim": dim})


@app.post("/model/reset")
def model_reset_route():
    """Unload the DINO model — logout, or a subscription that lapsed."""
    pipeline.reset_embed_model()
    return jsonify({"ok": True})


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
    """Body: {query_path: str, candidate_paths: [str], mirror?: bool,
    priority?: 'search'|'index'}.

    `mirror` flips the query horizontally before SIFT — see
    `pipeline.prepare_query` for why that is the only way a mirrored match
    can be found. Defaults false, so an older caller is unaffected."""
    body = request.get_json(force=True) or {}
    query_path = body.get("query_path")
    candidates = body.get("candidate_paths")
    if not query_path or not candidates:
        return jsonify({"error": "query_path and candidate_paths are required"}), 400
    payload = {"query_path": query_path, "candidate_paths": candidates,
               "mirror": bool(body.get("mirror", False))}
    return jsonify(_submit("verify", payload, body.get("priority", "search")))


def main() -> None:
    threading.Thread(target=_worker, daemon=True).start()
    port = int(os.environ.get("SIDECAR_PORT", "8756"))
    log.info("sidecar listening on 127.0.0.1:%d", port)
    serve(app, host="127.0.0.1", port=port, threads=8)


if __name__ == "__main__":
    main()
