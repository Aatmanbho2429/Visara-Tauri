# Evaluation harness for the exact-match engine's feature verification (SIFT today); standalone, needs only cv2/numpy.
# Run: python tools/eval_exact_match.py --library <folder> --n 200 --negatives 20 --seed 0 --out report.json
from __future__ import annotations

import argparse
import json
import os
import sys
import tempfile
import time

import cv2
import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

import pipeline  # noqa: E402

# Copy thresholds — mirror Rust `config::MATCH_COPY_MIN_INLIERS` / `MATCH_COPY_MIN_RATIO`; uncalibrated starting points.
COPY_MIN_INLIERS = 25
COPY_MIN_RATIO = 0.5
IMAGE_EXTS = (".jpg", ".jpeg", ".png")


# Centre crop keeping `area` of the pixels.
def _crop(img, area):
    h, w = img.shape[:2]
    f = float(np.sqrt(area))
    ch, cw = max(8, int(h * f)), max(8, int(w * f))
    y0, x0 = (h - ch) // 2, (w - cw) // 2
    return np.ascontiguousarray(img[y0:y0 + ch, x0:x0 + cw])


# Hue rotated by 60 degrees: a stand-in for a colourway of the same design.
def _hue_shift(img):
    hsv = cv2.cvtColor(img, cv2.COLOR_RGB2HSV)
    hsv[..., 0] = (hsv[..., 0].astype(np.int32) + 30) % 180
    return cv2.cvtColor(hsv, cv2.COLOR_HSV2RGB)


# JPEG round trip at quality 60.
def _jpeg60(img):
    ok, buf = cv2.imencode(".jpg", cv2.cvtColor(img, cv2.COLOR_RGB2BGR), [cv2.IMWRITE_JPEG_QUALITY, 60])
    return cv2.cvtColor(cv2.imdecode(buf, cv2.IMREAD_COLOR), cv2.COLOR_BGR2RGB)


# name -> function(RGB uint8) -> RGB uint8, every transform the plan lists.
TRANSFORMS = {
    "resize_0.5x": lambda im: cv2.resize(im, None, fx=0.5, fy=0.5, interpolation=cv2.INTER_AREA),
    "jpeg_q60": _jpeg60,
    "rot90": lambda im: np.ascontiguousarray(np.rot90(im, 1)),
    "rot180": lambda im: np.ascontiguousarray(np.rot90(im, 2)),
    "mirror": lambda im: np.ascontiguousarray(im[:, ::-1]),
    "crop_50pct": lambda im: _crop(im, 0.50),
    "crop_25pct": lambda im: _crop(im, 0.25),
    "brightness_+20": lambda im: cv2.convertScaleAbs(im, alpha=1.0, beta=51),
    "brightness_-20": lambda im: cv2.convertScaleAbs(im, alpha=1.0, beta=-51),
    "contrast_+20": lambda im: cv2.convertScaleAbs(im, alpha=1.2, beta=128 * (1 - 1.2)),
    "contrast_-20": lambda im: cv2.convertScaleAbs(im, alpha=0.8, beta=128 * (1 - 0.8)),
    "hue_shift": _hue_shift,
}


# Hamming distance between two hex pHash strings.
def _ham(a, b):
    return bin(int(a, 16) ^ int(b, 16)).count("1")


# Per-candidate stored-style features for a library file: (pts, des float32, (h, w), feature bytes, phash0) or None.
def _candidate(path, cache):
    if path in cache:
        return cache[path]
    img = pipeline.load_image_rgb(path, max_dim=pipeline.MATCH_FEATURE_MAX_DIM)
    if img is None:
        cache[path] = None
        return None
    pts, _sizes, _angles, desc, (w, h) = pipeline.extract_features(img)
    fp = pipeline.fingerprint(pipeline.load_image_rgb(path, max_dim=512))
    n = len(pts)
    cache[path] = (pts, desc.astype(np.float32), (h, w), n * (8 + pipeline.SIFT_DIM), fp[0] if fp else None)
    return cache[path]


# Verifies one query (written to `qpath`) against one candidate the way search does: straight, then mirrored if that fails.
def _verify(qstates, cand):
    pts, des, shape, _bytes, _fp = cand
    best = None
    for state in qstates:
        if state is None or state["des"] is None or len(state["kp"]) < 8:
            continue
        r = pipeline._verify_matches(state["qg"].shape, state["pts"], state["des"], pts, des, shape, bool(state["mirror"]))
        if best is None or (r.get("matched") and not best.get("matched")) or (r.get("matched") == best.get("matched") and r.get("inliers", 0) > best.get("inliers", 0)):
            best = r
        if r.get("matched"):
            break
    return best or {"matched": False, "reason": "no-query-features"}


# True when a verify result clears the copy thresholds.
def _is_copy(r):
    return bool(r.get("matched")) and r.get("inliers", 0) >= COPY_MIN_INLIERS and r.get("inlier_ratio", 0.0) >= COPY_MIN_RATIO


# Runs the evaluation and returns the report dict.
def evaluate(library, n=200, negatives=20, seed=0, detector="sift"):
    if detector != "sift":
        raise NotImplementedError(f"detector '{detector}' is not implemented; only 'sift' is (XFeat needs vendored weights plus model code)")
    files = sorted(os.path.join(root, f) for root, _d, fs in os.walk(library) for f in fs if f.lower().endswith(IMAGE_EXTS))
    if len(files) < 2:
        raise SystemExit(f"need at least 2 images under {library}, found {len(files)}")
    rng = np.random.default_rng(seed)
    picks = [files[i] for i in rng.permutation(len(files))[:min(n, len(files))]]
    cache = {}
    stats = {name: {"tp": 0, "n": 0, "false_copy": 0, "neg_n": 0, "ms": [], "bytes": [], "ph_src": [], "ph_neg": []} for name in TRANSFORMS}
    with tempfile.TemporaryDirectory() as tmp:
        for src in picks:
            base = pipeline.load_image_rgb(src, max_dim=pipeline.MATCH_FEATURE_MAX_DIM)
            source = _candidate(src, cache)
            if base is None or source is None:
                continue
            others = [f for f in files if f != src]
            neg_paths = [others[i] for i in rng.permutation(len(others))[:min(negatives, len(others))]]
            negs = [c for c in (_candidate(p, cache) for p in neg_paths) if c is not None]
            for name, fn in TRANSFORMS.items():
                q = fn(base)
                qpath = os.path.join(tmp, "q.png")
                cv2.imwrite(qpath, cv2.cvtColor(q, cv2.COLOR_RGB2BGR))
                qstates = [pipeline.prepare_query(qpath, mirror=False), pipeline.prepare_query(qpath, mirror=True)]
                st = stats[name]
                st["n"] += 1
                st["bytes"].append(source[3])
                t0 = time.perf_counter()
                r = _verify(qstates, source)
                st["ms"].append((time.perf_counter() - t0) * 1000)
                st["tp"] += int(bool(r.get("matched")))
                qfp = pipeline.fingerprint(q)
                if qfp and source[4]:
                    st["ph_src"].append(min(_ham(h, source[4]) for h in qfp))
                for cand in negs:
                    t0 = time.perf_counter()
                    rn = _verify(qstates, cand)
                    st["ms"].append((time.perf_counter() - t0) * 1000)
                    st["neg_n"] += 1
                    st["false_copy"] += int(_is_copy(rn))
                    if qfp and cand[4]:
                        st["ph_neg"].append(min(_ham(h, cand[4]) for h in qfp))
    report = {"config": {"detector": detector, "n_requested": n, "negatives": negatives, "seed": seed, "library_files": len(files),
                         "copy_min_inliers": COPY_MIN_INLIERS, "copy_min_ratio": COPY_MIN_RATIO,
                         "features_per_image": pipeline.MATCH_FEATURES_PER_IMAGE}, "transforms": {}}
    for name, st in stats.items():
        report["transforms"][name] = {
            "n": st["n"],
            "true_positive_rate": (st["tp"] / st["n"]) if st["n"] else None,
            "false_copy_rate": (st["false_copy"] / st["neg_n"]) if st["neg_n"] else None,
            "verify_ms_median": float(np.median(st["ms"])) if st["ms"] else None,
            "verify_ms_p95": float(np.percentile(st["ms"], 95)) if st["ms"] else None,
            "feature_bytes_mean": float(np.mean(st["bytes"])) if st["bytes"] else None,
            "phash_source_median": float(np.median(st["ph_src"])) if st["ph_src"] else None,
            "phash_negative_median": float(np.median(st["ph_neg"])) if st["ph_neg"] else None,
            "phash_negative_min": int(min(st["ph_neg"])) if st["ph_neg"] else None,
        }
    return report


def main(argv=None):
    ap = argparse.ArgumentParser(description="Evaluate exact-match feature verification on synthesised queries")
    ap.add_argument("--library", required=True, help="folder of library images (searched recursively)")
    ap.add_argument("--n", type=int, default=200, help="random source images to synthesise queries from")
    ap.add_argument("--negatives", type=int, default=20, help="random other images each query is also verified against")
    ap.add_argument("--seed", type=int, default=0)
    ap.add_argument("--out", default="report.json")
    ap.add_argument("--detector", default="sift")
    args = ap.parse_args(argv)
    try:
        report = evaluate(args.library, args.n, args.negatives, args.seed, args.detector)
    except NotImplementedError as e:
        print(e, file=sys.stderr)
        return 2
    with open(args.out, "w") as f:
        json.dump(report, f, indent=1)
    print(f"{'transform':<16}{'n':>5}{'TPR':>8}{'falseCopy':>11}{'med ms':>9}{'p95 ms':>9}{'bytes':>9}{'ph src':>8}{'ph neg':>8}")
    for name, r in report["transforms"].items():
        fmt = lambda v, p=2: "-" if v is None else f"{v:.{p}f}"  # noqa: E731
        print(f"{name:<16}{r['n']:>5}{fmt(r['true_positive_rate']):>8}{fmt(r['false_copy_rate'], 4):>11}"
              f"{fmt(r['verify_ms_median']):>9}{fmt(r['verify_ms_p95']):>9}{fmt(r['feature_bytes_mean'], 0):>9}"
              f"{fmt(r['phash_source_median'], 1):>8}{fmt(r['phash_negative_median'], 1):>8}")
    print("wrote", args.out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
