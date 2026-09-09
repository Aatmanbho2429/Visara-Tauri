#!/usr/bin/env python3
"""Pictoria test harness — drives the live app's sidecar, store and logs.

Run with the sidecar's own venv interpreter (it already has PIL/numpy); only
stdlib is used for HTTP so nothing extra needs installing.
"""

import argparse
import glob
import json
import os
import re
import sqlite3
import struct
import subprocess
import sys
import time
import unicodedata
import urllib.error
import urllib.request

import numpy as np
from PIL import Image

SIDECAR = "http://127.0.0.1:8756"
HOME = os.path.expanduser("~")
DB_PATH = os.path.join(HOME, ".pictoria", "meta.db")
STORE_PATH = os.path.join(HOME, ".pictoria", "vectors.bin")
LOG_PATH = os.path.join(HOME, "Library", "Logs", "com.pictoria.app", "Pictoria.log")
HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", "..", ".."))

# Mirrors search_service.rs::trim_uniform_border — see trim_like_app below.
MAX_TRIM = 0.25
TRIM_TOL = 12

# pipeline.verify_one's own hard floor. Reported as the margin every match
# has left, because a match sitting on top of it is one SIFT tuning change
# away from disappearing.
PIPELINE_MIN_INLIERS = 10


# ── Reporting ─────────────────────────────────────────────────────────────

class Report:
    def __init__(self):
        self.rows = []

    # `detail` is printed under the row; `fragile` marks a check that passed
    # but sits close enough to its threshold to be worth watching.
    def add(self, suite, name, status, detail="", fragile=False):
        self.rows.append((suite, name, status, detail, fragile))
        icon = {"PASS": "\033[32m✓\033[0m", "FAIL": "\033[31m✗\033[0m",
                "SKIP": "\033[33m−\033[0m", "INFO": "\033[36mi\033[0m"}[status]
        warn = " \033[33m[fragile]\033[0m" if fragile else ""
        print(f"  {icon} {name}{warn}")
        if detail:
            for line in str(detail).splitlines():
                print(f"      {line}")

    def counts(self):
        c = {"PASS": 0, "FAIL": 0, "SKIP": 0, "INFO": 0}
        for r in self.rows:
            c[r[2]] += 1
        return c

    def failed(self):
        return [r for r in self.rows if r[2] == "FAIL"]


# ── Database access ───────────────────────────────────────────────────────

# meta.db is in WAL mode and the app writes to it while this runs, so a
# read-only connection can intermittently fail to attach the -shm file. Try
# the safe modes first and fall back to reading a copy — never open the live
# database read-write, because a test run must not be able to touch it.
def db_connect():
    attempts = [
        (f"file:{DB_PATH}?mode=ro", True),
        (f"file:{DB_PATH}?mode=ro&immutable=1", True),
    ]
    last = None
    for uri, is_uri in attempts:
        try:
            con = sqlite3.connect(uri, uri=is_uri, timeout=5)
            con.execute("SELECT 1 FROM files LIMIT 1")
            return con
        except Exception as e:
            last = e
    import shutil
    tmp = os.path.join("/tmp", f"pictoria_meta_{os.getpid()}.db")
    for suffix in ("", "-wal", "-shm"):
        src = DB_PATH + suffix
        if os.path.exists(src):
            shutil.copy2(src, tmp + suffix)
    try:
        return sqlite3.connect(tmp, timeout=5)
    except Exception:
        raise last


# ── Sidecar HTTP ──────────────────────────────────────────────────────────

def sidecar_get(path, timeout=5):
    with urllib.request.urlopen(f"{SIDECAR}{path}", timeout=timeout) as r:
        return json.load(r)


# Suites can be run individually, so each one that needs the sidecar checks
# for itself rather than relying on preflight having populated ctx.
def ensure_sidecar(ctx):
    if "embed_ready" not in ctx:
        try:
            h = sidecar_get("/health")
            ctx["embed_ready"] = bool(h.get("embed_ready"))
            ctx["abort_sidecar"] = False
        except Exception:
            ctx["embed_ready"] = False
            ctx["abort_sidecar"] = True
    return not ctx.get("abort_sidecar") and ctx.get("embed_ready")


def sidecar_post(path, payload, timeout=180):
    req = urllib.request.Request(
        f"{SIDECAR}{path}",
        data=json.dumps(payload).encode(),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.load(r)


# ── Path resolution ───────────────────────────────────────────────────────

# Library filenames contain U+202F (narrow no-break space) that does not
# survive being retyped, so a fixture name is resolved by exact hit first,
# then a glob with every space widened to `?`, then a whitespace-normalised
# scan. Never compare fixture strings to filenames directly.
def resolve(library, name):
    exact = os.path.join(library, name)
    if os.path.exists(exact):
        return exact
    widened = os.path.join(library, re.sub(r"\s", "?", name))
    hits = glob.glob(widened)
    if len(hits) == 1:
        return hits[0]
    if len(hits) > 1:
        return sorted(hits)[0]
    want = _norm_ws(name)
    for root, _dirs, files in os.walk(library):
        for f in files:
            rel = os.path.relpath(os.path.join(root, f), library)
            if _norm_ws(rel) == want:
                return os.path.join(root, f)
    return None


def _norm_ws(s):
    s = unicodedata.normalize("NFC", s)
    return re.sub(r"\s+", " ", s).strip().lower()


# ── Query trimming (must match the Rust) ──────────────────────────────────

# Port of search_service.rs::trim_uniform_border. The app trims a uniform
# scanner/product-shot margin off the query before describing or verifying
# it, and that crop moves inlier counts materially (the canonical `big
# image.png` case went 12 -> 11 inliers from a ~2px crop alone), so a test
# that verifies the raw file is testing something the app never does.
def trim_like_app(path, out_path):
    img = Image.open(path).convert("RGB")
    a = np.asarray(img, dtype=np.int32)
    h, w = a.shape[0], a.shape[1]
    if w < 8 or h < 8:
        img.save(out_path)
        return out_path, False

    corners = np.stack([a[0, 0], a[0, w - 1], a[h - 1, 0], a[h - 1, w - 1]])
    refc = corners.sum(axis=0) // 4

    step_x = max(w // 64, 1)
    step_y = max(h // 64, 1)

    def row_is_margin(y):
        return bool(np.all(np.abs(a[y, ::step_x] - refc) <= TRIM_TOL))

    def col_is_margin(x):
        return bool(np.all(np.abs(a[::step_y, x] - refc) <= TRIM_TOL))

    max_y = int(h * MAX_TRIM)
    max_x = int(w * MAX_TRIM)

    top = 0
    while top < max_y and row_is_margin(top):
        top += 1
    bottom = 0
    while bottom < max_y and row_is_margin(h - 1 - bottom):
        bottom += 1
    left = 0
    while left < max_x and col_is_margin(left):
        left += 1
    right = 0
    while right < max_x and col_is_margin(w - 1 - right):
        right += 1

    if top == 0 and bottom == 0 and left == 0 and right == 0:
        img.save(out_path)
        return out_path, False
    if top >= max_y and bottom >= max_y and left >= max_x and right >= max_x:
        img.save(out_path)
        return out_path, False

    nw = w - (left + right)
    nh = h - (top + bottom)
    if nw < w // 2 or nh < h // 2 or nw < 8 or nh < 8:
        img.save(out_path)
        return out_path, False

    img.crop((left, top, left + nw, top + nh)).save(out_path)
    return out_path, True


# ── Vector store reader ───────────────────────────────────────────────────

MAGIC = b"PICTOR\x00\x01"
TOMBSTONE = -(2 ** 63)


# Parses vectors.bin per the layout documented in core/vector_store.rs.
def read_store(path, cfg):
    with open(path, "rb") as f:
        raw = f.read()
    if len(raw) < 36 or raw[:8] != MAGIC:
        raise ValueError("bad magic / truncated header")
    version = struct.unpack_from("<I", raw, 8)[0]
    rose_dim = struct.unpack_from("<I", raw, 12)[0]
    count = struct.unpack_from("<Q", raw, 16)[0]
    gram_dim = struct.unpack_from("<I", raw, 24)[0]
    color_dim = struct.unpack_from("<I", raw, 28)[0]
    embed_dim = struct.unpack_from("<I", raw, 32)[0]
    body = raw[36:]
    stride = 8 + (embed_dim + rose_dim + gram_dim + color_dim) * 4
    if stride == 0 or len(body) % stride:
        raise ValueError(f"body {len(body)} not a multiple of stride {stride}")
    n = len(body) // stride

    ids = np.empty(n, dtype=np.int64)
    embeds = np.empty((n, embed_dim), dtype=np.float32)
    for i in range(n):
        off = i * stride
        ids[i] = struct.unpack_from("<q", body, off)[0]
        embeds[i] = np.frombuffer(body, dtype="<f4", count=embed_dim, offset=off + 8)

    return {
        "version": version, "count_header": count, "n_entries": n, "stride": stride,
        "rose_dim": rose_dim, "gram_dim": gram_dim, "color_dim": color_dim,
        "embed_dim": embed_dim, "ids": ids, "embeds": embeds,
    }


# Cross-zoom best cosine, matching vector_store.rs::best_zoom_dot. Both sides
# are unit-normalised per zoom level, so a dot product is the cosine.
def best_zoom_dot(query, candidates, per_zoom):
    zq = query.reshape(-1, per_zoom)
    zq = zq / np.maximum(np.linalg.norm(zq, axis=1, keepdims=True), 1e-12)
    zc = candidates.reshape(candidates.shape[0], -1, per_zoom)
    zc = zc / np.maximum(np.linalg.norm(zc, axis=2, keepdims=True), 1e-12)
    return np.einsum("qd,ncd->nqc", zq, zc).reshape(candidates.shape[0], -1).max(axis=1)


# ── Suites ────────────────────────────────────────────────────────────────

def suite_preflight(rep, ctx):
    print("\n\033[1mpreflight\033[0m")
    ps = subprocess.run(["ps", "ax", "-o", "command"], capture_output=True, text=True).stdout
    app_up = "pictoria" in ps.lower() and "tauri" in ps.lower()
    rep.add("preflight", "app process running",
            "PASS" if app_up else "SKIP",
            "" if app_up else "no `tauri dev` / Pictoria process found — sidecar tests may still work if it is running standalone")

    try:
        health = sidecar_get("/health")
    except Exception as e:
        rep.add("preflight", "sidecar /health reachable", "FAIL", f"{e}\nStart the app first — every sidecar suite needs it.")
        ctx["abort_sidecar"] = True
        return
    rep.add("preflight", "sidecar /health reachable", "PASS", json.dumps(health))

    rep.add("preflight", "gram model ready", "PASS" if health.get("ready") else "FAIL",
            "" if health.get("ready") else "sidecar up but mobilenet not loaded")
    embed_ok = bool(health.get("embed_ready"))
    rep.add("preflight", "DINO embed model loaded", "PASS" if embed_ok else "FAIL",
            "" if embed_ok else "log in to the app — /describe returns embed-model-not-loaded until auth pushes the licence key")
    ctx["embed_ready"] = embed_ok

    cfg = ctx["cases"]["config"]
    dim_ok = health.get("embed_dim") == cfg["embed_dim"]
    rep.add("preflight", f"embed_dim == config::EMBED_DIM ({cfg['embed_dim']})",
            "PASS" if dim_ok else "FAIL",
            "" if dim_ok else f"sidecar reports {health.get('embed_dim')} — a mismatch writes a wrong stride into vectors.bin")

    if not os.path.exists(DB_PATH):
        rep.add("preflight", "meta.db present", "FAIL", DB_PATH)
        return
    con = db_connect()
    folders = con.execute("SELECT path, status FROM watched_folders").fetchall()
    n_files = con.execute("SELECT COUNT(*) FROM files").fetchone()[0]
    con.close()
    rep.add("preflight", "library indexed", "PASS" if n_files else "FAIL",
            f"{n_files} files across {len(folders)} watched folder(s): " +
            ", ".join(f"{p} [{s}]" for p, s in folders))
    ctx["n_files"] = n_files


def suite_describe(rep, ctx):
    print("\n\033[1mdescribe\033[0m  (sidecar /describe contract)")
    if not ensure_sidecar(ctx):
        rep.add("describe", "all", "SKIP", "sidecar unreachable or embed model not loaded")
        return
    cfg = ctx["cases"]["config"]
    q = ctx["query_trimmed"]

    t0 = time.time()
    res = sidecar_post("/describe", {"paths": [q], "priority": "search"})["results"][0]
    ms = (time.time() - t0) * 1000

    if res.get("error"):
        rep.add("describe", "describes the query", "FAIL", res["error"])
        return
    rep.add("describe", "describes the query", "PASS", f"{ms:.0f} ms")

    checks = [
        ("embed", cfg["embed_zoom_levels"], cfg["embed_dim"]),
        ("gram", cfg["gram_zoom_levels"], cfg["gram_dim_per_zoom"]),
    ]
    for key, levels, width in checks:
        v = res.get(key)
        ok = isinstance(v, list) and len(v) == levels and all(len(z) == width for z in v)
        rep.add("describe", f"{key} shape == {levels}x{width}", "PASS" if ok else "FAIL",
                "" if ok else f"got {len(v)}x{len(v[0]) if v else '?'}")

    rose_ok = len(res.get("rose", [])) == cfg["rose_dim"]
    rep.add("describe", f"rose width == {cfg['rose_dim']}", "PASS" if rose_ok else "FAIL")
    color_ok = len(res.get("color", [])) == cfg["color_dim"]
    rep.add("describe", f"color width == {cfg['color_dim']}", "PASS" if color_ok else "FAIL",
            "" if color_ok else f"got {len(res.get('color', []))}")

    # Each embedding zoom level must already be unit length — this is the
    # precondition vector_store.rs::best_zoom_dot relies on to substitute a
    # dot product for a full cosine.
    norms = [float(np.linalg.norm(np.array(z, dtype=np.float32))) for z in res["embed"]]
    unit = all(abs(n - 1.0) < 0.02 for n in norms)
    rep.add("describe", "embed zoom levels are unit-normalised", "PASS" if unit else "FAIL",
            "norms: " + ", ".join(f"{n:.4f}" for n in norms))

    # Same input twice must give the same vector, or the store and the query
    # path disagree and ranking drifts between index time and search time.
    res2 = sidecar_post("/describe", {"paths": [q], "priority": "search"})["results"][0]
    same = np.allclose(np.array(res["embed"], dtype=np.float32),
                       np.array(res2["embed"], dtype=np.float32), atol=1e-6)
    rep.add("describe", "describe is deterministic", "PASS" if same else "FAIL")

    bogus = sidecar_post("/describe", {"paths": ["/nonexistent/nope.png"], "priority": "search"})["results"][0]
    rep.add("describe", "undecodable path fails softly", "PASS" if bogus.get("error") else "FAIL",
            f"error={bogus.get('error')}")
    ctx["query_embed"] = np.array(res["embed"], dtype=np.float32).reshape(-1)


def suite_verify(rep, ctx):
    print("\n\033[1mverify\033[0m  (SIFT/RANSAC regression set)")
    if not ensure_sidecar(ctx):
        rep.add("verify", "all", "SKIP", "sidecar unreachable or embed model not loaded")
        return
    lib = ctx["library"]

    for case in ctx["cases"]["verify_cases"]:
        qname = case["query"]
        qpath = resolve(lib, qname)
        if not qpath:
            rep.add("verify", f"{case['name']}: query present", "SKIP", f"not found: {qname}")
            continue
        trimmed = os.path.join(ctx["tmp"], f"q_{abs(hash(qpath))}.png")
        trim_like_app(qpath, trimmed)

        wanted = case["expect_match"] + case["expect_reject"]
        paths, missing = [], []
        for c in wanted:
            p = resolve(lib, c["candidate"])
            (paths.append(p) if p else missing.append(c["candidate"]))
        if missing:
            rep.add("verify", f"{case['name']}: candidates present", "SKIP",
                    "missing from library: " + ", ".join(missing))
        if not paths:
            continue

        out = sidecar_post("/verify", {"query_path": trimmed, "candidate_paths": paths,
                                       "mirror": False, "priority": "search"})
        by_path = {r["path"]: r for r in out["results"]}

        for c in case["expect_match"]:
            p = resolve(lib, c["candidate"])
            if not p:
                continue
            r = by_path.get(p, {})
            inl, ratio = r.get("inliers", 0), r.get("inlier_ratio", 0.0)

            # Hard contract: it matched, and the scale-free ratio holds up.
            # The absolute inlier count is deliberately NOT a gate — see the
            # assertion-model note in cases.json.
            ok = bool(r.get("matched")) and ratio >= c["min_ratio"]

            # PIPELINE_MIN_INLIERS is verify_one's own hard floor; anything
            # sitting on top of it is one tuning change away from vanishing.
            margin = inl - PIPELINE_MIN_INLIERS
            fragile = bool(ok and (inl < c.get("warn_below_inliers", 0) or margin <= 2))

            detail = (f"inliers={inl} (+{margin} over pipeline floor {PIPELINE_MIN_INLIERS})  "
                      f"ratio={ratio:.2f} (min {c['min_ratio']})  reason={r.get('reason', '-')}")
            if fragile:
                detail += f"\n  thin margin — baseline: {c.get('baseline', 'n/a')}"
            if not ok:
                detail += f"\n  {c['note']}\n  baseline: {c.get('baseline', 'n/a')}"
            rep.add("verify", f"{case['name']}: {os.path.basename(p)} verifies",
                    "PASS" if ok else "FAIL", detail, fragile)

        for c in case["expect_reject"]:
            p = resolve(lib, c["candidate"])
            if not p:
                continue
            r = by_path.get(p, {})
            ok = not r.get("matched")
            rep.add("verify", f"{case['name']}: {os.path.basename(p)} stays rejected",
                    "PASS" if ok else "FAIL",
                    f"matched={r.get('matched')} inliers={r.get('inliers', 0)} reason={r.get('reason', '-')}")

    _colourway_pairs(rep, ctx)


# Every pair in a colourway group is the same design recoloured, so all of
# them must verify. Recolouring destroys most SIFT correspondences, so this
# is what catches a change that quietly starts gating matches on colour.
def _colourway_pairs(rep, ctx):
    grp = ctx["cases"]["colourway_group"]
    files = sorted(glob.glob(os.path.join(ctx["library"], grp["glob"])))
    if len(files) < 2:
        rep.add("verify", grp["name"], "SKIP", f"found {len(files)} files for {grp['glob']}")
        return

    pairs, ok_pairs, failures = [], 0, []
    for i in range(len(files)):
        for j in range(i + 1, len(files)):
            pairs.append((files[i], files[j]))

    for a, b in pairs:
        trimmed = os.path.join(ctx["tmp"], f"cw_{abs(hash(a))}.png")
        if not os.path.exists(trimmed):
            trim_like_app(a, trimmed)
        try:
            out = sidecar_post("/verify", {"query_path": trimmed, "candidate_paths": [b],
                                           "mirror": False, "priority": "search"})
            r = out["results"][0]
            if r.get("matched"):
                ok_pairs += 1
            else:
                failures.append(f"{os.path.basename(a)} -> {os.path.basename(b)}: "
                                f"{r.get('reason', '?')} (inliers={r.get('inliers', 0)})")
        except Exception as e:
            failures.append(f"{os.path.basename(a)} -> {os.path.basename(b)}: {e}")

    passed = ok_pairs == len(pairs) and len(pairs) >= grp.get("min_pairs", 1)
    detail = f"{ok_pairs}/{len(pairs)} pairs verified"
    if failures:
        detail += "\n" + "\n".join(failures[:8])
    rep.add("verify", grp["name"], "PASS" if passed else "FAIL", detail)


def suite_store(rep, ctx):
    print("\n\033[1mstore\033[0m  (vectors.bin ↔ meta.db integrity)")
    cfg = ctx["cases"]["config"]
    if not os.path.exists(STORE_PATH):
        rep.add("store", "vectors.bin present", "FAIL", STORE_PATH)
        return
    try:
        st = read_store(STORE_PATH, cfg)
    except Exception as e:
        rep.add("store", "vectors.bin parses", "FAIL", str(e))
        return
    rep.add("store", "vectors.bin parses", "PASS",
            f"version={st['version']} entries={st['n_entries']} stride={st['stride']}B "
            f"({os.path.getsize(STORE_PATH) / 1e6:.1f} MB)")

    ver_ok = st["version"] == cfg["store_version"]
    rep.add("store", f"format version == {cfg['store_version']}", "PASS" if ver_ok else "FAIL",
            "" if ver_ok else f"got {st['version']} — a mismatch makes load() start fresh and arms a full re-index")

    want_embed = cfg["embed_dim"] * cfg["embed_zoom_levels"]
    want_gram = cfg["gram_dim_per_zoom"] * cfg["gram_zoom_levels"]
    dims_ok = (st["embed_dim"] == want_embed and st["gram_dim"] == want_gram
               and st["rose_dim"] == cfg["rose_dim"] and st["color_dim"] == cfg["color_dim"])
    rep.add("store", "stored dims match config", "PASS" if dims_ok else "FAIL",
            f"embed={st['embed_dim']}/{want_embed} gram={st['gram_dim']}/{want_gram} "
            f"rose={st['rose_dim']}/{cfg['rose_dim']} color={st['color_dim']}/{cfg['color_dim']}")

    live_mask = st["ids"] != TOMBSTONE
    live = st["ids"][live_mask]
    dupes = len(live) - len(np.unique(live))
    rep.add("store", "no duplicate live ids", "PASS" if dupes == 0 else "FAIL",
            f"{len(live)} live, {st['n_entries'] - len(live)} tombstoned, {dupes} duplicates")

    # The Phase 5b precondition, checked against real stored data rather than
    # the producer's promise: near_family uses a dot product for the
    # embedding, which only equals the cosine when every stored zoom level is
    # already unit length. A debug_assert covers this in debug builds only —
    # this is the release-build check.
    emb = st["embeds"][live_mask].reshape(len(live), cfg["embed_zoom_levels"], cfg["embed_dim"])
    norms = np.linalg.norm(emb, axis=2)
    worst = float(np.max(np.abs(norms - 1.0))) if norms.size else 0.0
    bad = int(np.sum(np.abs(norms - 1.0) > 0.05))
    rep.add("store", "every stored embed zoom level is unit-normalised",
            "PASS" if bad == 0 else "FAIL",
            f"worst deviation {worst:.5f} across {norms.size} zoom vectors; {bad} outside ±0.05"
            + ("" if bad == 0 else "\n  best_zoom_dot will mis-score these entries — re-index to rewrite them through upsert()"))

    if os.path.exists(DB_PATH):
        con = db_connect()
        db_ids = {r[0] for r in con.execute("SELECT faiss_id FROM files")}
        n_paths = con.execute("SELECT COUNT(DISTINCT path) FROM files").fetchone()[0]
        n_rows = con.execute("SELECT COUNT(*) FROM files").fetchone()[0]
        con.close()
        store_ids = set(int(i) for i in live)
        orphan_store = store_ids - db_ids
        orphan_db = db_ids - store_ids
        ok = not orphan_store and not orphan_db
        rep.add("store", "meta.db ids ↔ vectors.bin ids", "PASS" if ok else "FAIL",
                f"{len(store_ids)} in store, {len(db_ids)} in db, "
                f"{len(orphan_store)} store-only, {len(orphan_db)} db-only")
        rep.add("store", "no duplicate paths in meta.db", "PASS" if n_paths == n_rows else "FAIL",
                f"{n_rows} rows, {n_paths} distinct paths")
    ctx["store"] = st


def suite_nearfamily(rep, ctx):
    print("\n\033[1mnearfamily\033[0m  (stage-1 retrieval, computed independently of the app)")
    cfg = ctx["cases"]["config"]
    nf = ctx["cases"]["near_family"]
    st = ctx.get("store")
    if st is None:
        rep.add("nearfamily", "all", "SKIP", "store did not parse")
        return
    if ctx.get("query_embed") is None:
        if not ensure_sidecar(ctx):
            rep.add("nearfamily", "all", "SKIP", "sidecar unreachable or embed model not loaded")
            return
        r = sidecar_post("/describe", {"paths": [ctx["query_trimmed"]], "priority": "search"})["results"][0]
        if r.get("error"):
            rep.add("nearfamily", "all", "SKIP", f"describe failed: {r['error']}")
            return
        ctx["query_embed"] = np.array(r["embed"], dtype=np.float32).reshape(-1)

    live_mask = st["ids"] != TOMBSTONE
    sims = best_zoom_dot(ctx["query_embed"], st["embeds"][live_mask], cfg["embed_dim"])
    ids = st["ids"][live_mask]

    con = db_connect()
    id_to_path = dict(con.execute("SELECT faiss_id, path FROM files"))
    con.close()

    order = np.argsort(-sims)
    floor = cfg["near_family_min_sim"]
    family = [(float(sims[i]), id_to_path.get(int(ids[i]), f"<id {ids[i]}>")) for i in order if sims[i] >= floor]

    pct = len(family) / max(len(sims), 1) * 100
    rep.add("nearfamily", f"near family at cosine >= {floor}", "INFO",
            f"{len(family)} of {len(sims)} live files ({pct:.1f}% of the library)"
            + ("\n  a large fraction here means the floor is too loose — see the histogram below" if pct > 40 else ""))

    # Same buckets the app's [calibration] line reports, computed here so the
    # threshold can be set without instrumenting a build.
    buckets = [0.60, 0.65, 0.70, 0.75, 0.80, 0.85, 0.90, 0.95]
    hist = "  ".join(f"{b:.2f}={int(np.sum(sims >= b))}" for b in buckets)
    rep.add("nearfamily", "cosine distribution", "INFO", hist)

    top = "\n".join(f"{s:.4f}  {os.path.basename(p)}" for s, p in family[:8])
    rep.add("nearfamily", "top of the family", "INFO", top or "(empty)")

    # Stage 1 has to surface the known matches, or verification never gets a
    # chance to prove them — a silent recall failure the UI cannot show.
    fam_paths = {_norm_ws(os.path.basename(p)) for _s, p in family}
    for want in nf["must_contain"]:
        target = resolve(ctx["library"], want)
        key = _norm_ws(os.path.basename(target)) if target else _norm_ws(want)
        found = key in fam_paths
        rank = next((i + 1 for i, (_s, p) in enumerate(family) if _norm_ws(os.path.basename(p)) == key), None)
        sim = next((s for s, p in family if _norm_ws(os.path.basename(p)) == key), None)
        rep.add("nearfamily", f"{os.path.basename(want)} is in the near family",
                "PASS" if found else "FAIL",
                f"rank {rank}, cosine {sim:.4f}" if found else
                f"below the {floor} floor — stage 1 would never send it to verification")


def suite_latency(rep, ctx):
    print("\n\033[1mlatency\033[0m  (parsed from the app's own [timing] log)")
    cfg = ctx["cases"]["config"]
    if not os.path.exists(LOG_PATH):
        rep.add("latency", "log present", "SKIP", LOG_PATH)
        return
    with open(LOG_PATH, "r", errors="replace") as f:
        lines = f.readlines()

    searches = [l for l in lines if "[timing] search TOTAL" in l]
    if not searches:
        rep.add("latency", "search timings found", "SKIP",
                "no search has run since this log rotated — run one in the app, then re-run this suite")
        return

    def fields(line):
        return {k: v for k, v in re.findall(r"(\w+)=([\d.]+|true|false)", line)}

    recent = [fields(l) for l in searches[-5:]]
    rows = []
    for f in recent:
        rows.append(
            f"total={float(f.get('total_ms', 0)):8.0f}ms  "
            f"near_family_n={f.get('near_family_n', '?'):>5}  "
            f"verify_attempted_n={f.get('verify_attempted_n', '?'):>4}  "
            f"verified_n={f.get('verified_n', '?'):>3}  "
            f"verify_ms={float(f.get('verify_ms', 0)):7.0f}  "
            f"describe_ms={float(f.get('describe_ms', 0)):6.0f}  "
            f"store_load_ms={float(f.get('store_load_ms', 0)):6.1f}"
        )
    rep.add("latency", f"last {len(recent)} searches", "INFO", "\n".join(rows))

    last = recent[-1]
    attempted = int(float(last.get("verify_attempted_n", 0)))
    cap = cfg["verify_max_candidates"]
    rep.add("latency", f"verify pool respects VERIFY_MAX_CANDIDATES ({cap})",
            "PASS" if attempted <= cap else "FAIL",
            f"verify_attempted_n={attempted}")

    if "verify_budget_exhausted" in last:
        exhausted = last["verify_budget_exhausted"] == "true"
        rep.add("latency", "verify budget not exhausted on the last search",
                "PASS" if not exhausted else "INFO",
                "budget was hit — the remainder is reported as unchecked, not rejected"
                if exhausted else "completed the whole pool within budget")

    rust_ms = sum(float(last.get(k, 0)) for k in ("store_load_ms", "id_map_ms", "near_family_ms"))
    rep.add("latency", "Rust-side hot path (store_load + id_map + near_family)", "INFO",
            f"{rust_ms:.1f} ms — Phase 0's decision gate is whether this stays under ~150 ms "
            f"at this library size in a release build")

    calib = [l for l in lines if "[calibration]" in l]
    if calib:
        rep.add("latency", "calibration lines", "INFO", "".join(calib[-2:]).strip())


def suite_unit(rep, ctx):
    print("\n\033[1munit\033[0m  (the repo's own tests)")
    r = subprocess.run(["cargo", "test", "--manifest-path",
                        os.path.join(REPO, "UI", "src-tauri", "Cargo.toml")],
                       capture_output=True, text=True)
    m = re.search(r"test result: (\w+)\. (\d+) passed; (\d+) failed", r.stdout)
    if m:
        rep.add("unit", "cargo test", "PASS" if m.group(3) == "0" else "FAIL",
                f"{m.group(2)} passed, {m.group(3)} failed")
    else:
        rep.add("unit", "cargo test", "FAIL", (r.stderr or r.stdout)[-600:])

    r = subprocess.run(["npm", "run", "build"], cwd=os.path.join(REPO, "UI"),
                       capture_output=True, text=True)
    ok = r.returncode == 0
    rep.add("unit", "npm run build", "PASS" if ok else "FAIL",
            "" if ok else (r.stderr or r.stdout)[-600:])


SUITES = {
    "preflight": suite_preflight,
    "describe": suite_describe,
    "verify": suite_verify,
    "store": suite_store,
    "nearfamily": suite_nearfamily,
    "latency": suite_latency,
    "unit": suite_unit,
}
DEFAULT = ["preflight", "describe", "verify", "store", "nearfamily", "latency"]


def main():
    ap = argparse.ArgumentParser(description="Pictoria test harness")
    ap.add_argument("suites", nargs="*", default=None,
                    help=f"any of: {', '.join(SUITES)} (default: everything except `unit`)")
    ap.add_argument("--library", help="watched folder to test against (default: read from meta.db)")
    ap.add_argument("--cases", default=os.path.join(HERE, "cases.json"))
    args = ap.parse_args()

    with open(args.cases) as f:
        cases = json.load(f)

    library = args.library
    if not library and os.path.exists(DB_PATH):
        con = db_connect()
        row = con.execute("SELECT path FROM watched_folders LIMIT 1").fetchone()
        con.close()
        library = row[0] if row else None
    if not library or not os.path.isdir(library):
        print(f"\033[31mNo library folder (looked for a watched folder in meta.db; got {library!r}).\033[0m")
        print("Pass one with --library.")
        return 2

    tmp = os.path.join("/tmp", f"pictoria_test_{os.getpid()}")
    os.makedirs(tmp, exist_ok=True)
    ctx = {"cases": cases, "library": library, "tmp": tmp}

    qpath = resolve(library, cases["near_family"]["query"])
    if qpath:
        ctx["query_trimmed"], was_trimmed = trim_like_app(qpath, os.path.join(tmp, "query.png"))
        print(f"library:  {library}")
        print(f"query:    {os.path.basename(qpath)}  (trimmed like the app: {was_trimmed})")
    else:
        ctx["query_trimmed"] = None
        print(f"\033[33mCanonical query not found in {library}; describe/nearfamily will skip.\033[0m")

    rep = Report()
    chosen = args.suites or DEFAULT
    for name in chosen:
        if name not in SUITES:
            print(f"unknown suite: {name}")
            return 2
        if name in ("describe", "nearfamily") and not ctx.get("query_trimmed"):
            continue
        SUITES[name](rep, ctx)

    c = rep.counts()
    print("\n" + "─" * 64)
    print(f"\033[1m{c['PASS']} passed, {c['FAIL']} failed, {c['SKIP']} skipped, {c['INFO']} informational\033[0m")
    if rep.failed():
        print("\nFailures:")
        for suite, name, _s, detail, _f in rep.failed():
            print(f"  \033[31m✗\033[0m {suite}: {name}")
            if detail:
                print(f"      {detail.splitlines()[0]}")
    return 1 if rep.failed() else 0


if __name__ == "__main__":
    sys.exit(main())
