"""Descriptor computation and geometric verification for the Pictoria sidecar.

Two jobs only, matching the HTTP API: DESCRIBE (Gabor + Gram-matrix, for
ranking) and VERIFY (SIFT + RANSAC, for proving an actual embedded crop).
Everything else — storage, ranking, caching — lives in Rust now.
"""
from __future__ import annotations

import logging
import os
import time
from dataclasses import dataclass

import cv2
import numpy as np
import torch
import torchvision

# Child of the "sidecar" logger configured in server.py — propagates to the
# same StreamHandler + FileHandler (sidecar/logs/sidecar.log), so these lines
# interleave with everything else on one timeline.
log = logging.getLogger("sidecar.pipeline")

_DEVICE = "cpu"
_LAYER_IDX = 4  # mobilenet_v2 block deep enough for texture, shallow enough to stay local
_GABOR_ORIENTS = 8
_GABOR_FREQS = (0.08, 0.15, 0.25)
_GRAM_ZOOM_SCALES = (0.75, 1.0, 1.4)  # keeps ranking robust to unrelated images being different pixel scales

_model = None
_mean = None
_std = None
_sift = None


def load_model():
    """Warm-load mobilenet_v2 once. Call at process startup, not per-request."""
    global _model, _mean, _std
    if _model is None:
        weights = torchvision.models.MobileNet_V2_Weights.IMAGENET1K_V1
        full = torchvision.models.mobilenet_v2(weights=weights).features.to(_DEVICE).eval()
        _model = torch.nn.Sequential(*list(full.children())[:_LAYER_IDX])
        _mean = torch.tensor(weights.transforms().mean).view(1, 3, 1, 1)
        _std = torch.tensor(weights.transforms().std).view(1, 3, 1, 1)
    return _model


def _new_sift():
    return cv2.SIFT_create(nfeatures=4000, contrastThreshold=0.01, edgeThreshold=20)


def _get_sift():
    global _sift
    if _sift is None:
        _sift = _new_sift()
    return _sift


def load_image_rgb(path, max_dim=512):
    """Decode a file to RGB, longest side capped at max_dim. Unicode-path safe."""
    try:
        data = np.fromfile(path, dtype=np.uint8)
        if data.size == 0:
            return None
        img = cv2.imdecode(data, cv2.IMREAD_COLOR)
    except Exception:
        return None
    if img is None:
        return None
    h, w = img.shape[:2]
    scale = max_dim / max(h, w)
    if scale < 1:
        img = cv2.resize(img, (int(w * scale), int(h * scale)), interpolation=cv2.INTER_AREA)
    return cv2.cvtColor(img, cv2.COLOR_BGR2RGB)


def gabor_rose(img_rgb):
    """8-bin orientation-energy histogram — which directions the grain runs."""
    gray = cv2.cvtColor(img_rgb, cv2.COLOR_RGB2GRAY).astype(np.float32) / 255.0
    bg = cv2.GaussianBlur(gray, (0, 0), 15)
    detail = gray - bg
    energies = np.zeros(_GABOR_ORIENTS, dtype=np.float64)
    for oi in range(_GABOR_ORIENTS):
        theta = oi * np.pi / _GABOR_ORIENTS
        for freq in _GABOR_FREQS:
            kernel = cv2.getGaborKernel((21, 21), sigma=4.0, theta=theta,
                                         lambd=1.0 / freq, gamma=0.6, psi=0)
            resp = cv2.filter2D(detail, cv2.CV_32F, kernel)
            energies[oi] += float(np.mean(resp ** 2))
    s = energies.sum()
    if s > 0:
        energies /= s
    return energies


@torch.no_grad()
def gram_descriptor(img_rgb, size=224, zoom_scales=_GRAM_ZOOM_SCALES):
    """Channel-correlation texture-style descriptor at a few zoom levels, so
    ranking isn't thrown off by two related images being different pixel scales.

    Fed a desaturated (grayscale, replicated to 3 channels) image rather than
    the original RGB — the CNN's ImageNet normalisation is otherwise very
    colour-sensitive, which used to bury same-design different-colourway
    files (e.g. a "small_yellow" variant of "small_blue") deep enough in the
    ranking that they'd fall out of the verify shortlist entirely, even
    though `gabor_rose` (already grayscale) recognised the shared structure
    fine. Matching that grayscale-only convention here makes ranking
    genuinely structure-led, with colour staying display-only via
    `color_histogram` — see [[pictoria-search-architecture]]."""
    model = load_model()
    gray = cv2.cvtColor(img_rgb, cv2.COLOR_RGB2GRAY)
    img_struct = cv2.cvtColor(gray, cv2.COLOR_GRAY2RGB)
    h0, w0 = img_struct.shape[:2]
    grams = []
    for zoom in zoom_scales:
        target_short = max(8, int(round(size / zoom)))
        scale = target_short / min(h0, w0)
        rh, rw = max(size, int(round(h0 * scale))), max(size, int(round(w0 * scale)))
        resized = cv2.resize(img_struct, (rw, rh), interpolation=cv2.INTER_AREA if scale < 1 else cv2.INTER_CUBIC)
        y0, x0 = (rh - size) // 2, (rw - size) // 2
        crop = resized[y0:y0 + size, x0:x0 + size]
        t = torch.from_numpy(crop).float().permute(2, 0, 1).unsqueeze(0) / 255.0
        t = (t - _mean) / _std
        feat = model(t)
        b, c, h, w = feat.shape
        f = feat.reshape(c, h * w)
        g = (f @ f.t()) / (c * h * w)
        grams.append(g.flatten().numpy())
    return grams


# ── Colour histogram ─────────────────────────────────────────────────────
# Vectorized port of `core::color::histogram` / `classify` (Rust, in
# UI/src-tauri/src/core/color.rs). Runs on the same decoded array `describe()`
# already produced for rose/gram — no second decode — which is the whole point
# of living here instead of Rust: colour used to pay for its own full-resolution
# decode in Rust just to build a 16-bucket histogram.
#
# `COLOR_BUCKETS` order MUST match `color.rs::COLOR_BUCKETS` exactly: it's
# positional in the persisted vector store, not just a label. The threshold
# logic in `_classify` MUST mirror `classify()` exactly too, or a query
# computed here would stop comparing like-for-like with a file indexed under
# the old Rust path (harmless for ranking — colour carries zero ranking
# weight — but it would drift the "colour match" badge shown in the UI).
COLOR_BUCKETS = (
    "white", "light grey", "grey", "charcoal", "black",
    "cream", "beige",
    "red", "brown", "terracotta", "gold",
    "green", "teal", "blue", "purple", "pink",
)
COLOR_DIM = len(COLOR_BUCKETS)
_COLOR_BUCKET_IDX = {name: i for i, name in enumerate(COLOR_BUCKETS)}
_HIST_MAX = 64  # matches Rust's HIST_MAX — long edge the image is shrunk to before counting


def _rgb_to_hsv_vec(rgb01):
    """rgb01: (N, 3) float64 in [0, 1] -> (h in [0, 360), s in [0, 1], v in [0, 1]).
    Mirrors `color::rgb_to_hsv` exactly, including its max-channel tie-break order
    (r, then g, then b) for which branch computes hue."""
    r, g, b = rgb01[:, 0], rgb01[:, 1], rgb01[:, 2]
    mx = np.maximum(np.maximum(r, g), b)
    mn = np.minimum(np.minimum(r, g), b)
    delta = mx - mn

    v = mx
    s = np.where(mx > 0, delta / np.where(mx > 0, mx, 1.0), 0.0)

    safe_delta = np.where(delta > 0, delta, 1.0)
    is_r = (mx == r) & (delta > 0)
    is_g = (~is_r) & (mx == g) & (delta > 0)
    h = np.where(
        is_r, 60.0 * (((g - b) / safe_delta) % 6.0),
        np.where(is_g, 60.0 * (((b - r) / safe_delta) + 2.0),
                 60.0 * (((r - g) / safe_delta) + 4.0)),
    )
    h = np.where(delta > 0, h, 0.0)
    h = np.where(h < 0, h + 360.0, h)
    return h, s, v


def _classify_vec(h, s, v):
    """Vectorized port of `color::classify` — bucket index per pixel. Conditions
    are evaluated in the same priority order as the Rust match/if-else chain
    (np.select picks the first true condition), so this must stay in lockstep
    with the arm order in color.rs, not just the arm set.

    `s` (saturation) alone is unreliable near black: it's a *ratio*
    (max-min)/max, so on a near-black pixel a tiny, often-imperceptible
    channel imbalance gets amplified into a "highly saturated" reading. A
    real "ABSOLUTE BLACK" granite tile has pixels like RGB (22,18,32) — a
    14/255 spread, invisible against that dark a background — yet
    s ≈ 0.44, comfortably past the 0.14 cutoff below, and because the blue
    channel is consistently a few units high across the tile (a genuine but
    imperceptible mineral/lighting cast, not sensor noise — verified
    against the raw pixels), the resulting hue lands in blue/purple almost
    every time. Measured: 93% of that tile's sampled pixels came out
    "purple"/"blue" this way, outvoting the correct black/charcoal call
    entirely. `chroma` (saturation × value — the *absolute* channel spread,
    not the ratio) fixes this without flattening genuinely deep colours:
    calibrated against 8 tiles literally named "black" (6 of 8 were
    misclassified before this) and 8 named "blue" including faint ones —
    0.06 is the highest floor that fixes every black tile while leaving
    every real blue tile, including a fainter slate-blue at chroma ≈0.10,
    untouched.
    """
    _MIN_CHROMA = 0.06
    chroma = s * v
    neutral = (s < 0.14) | (chroma < _MIN_CHROMA)
    muted = (~neutral) & (s < 0.34) & (v >= 0.5) & (h >= 20.0) & (h < 70.0)
    chromatic = (~neutral) & (~muted)

    conditions = [
        neutral & (v >= 0.85),                                # white
        neutral & (v >= 0.62),                                # light grey
        neutral & (v >= 0.32),                                # grey
        neutral & (v >= 0.12),                                # charcoal
        neutral,                                               # black (catch-all remainder of neutral)
        muted & (v >= 0.8),                                    # cream
        muted,                                                 # beige (catch-all remainder of muted)
        chromatic & ((h < 15.0) | (h >= 345.0)),               # red
        chromatic & (h >= 15.0) & (h < 45.0) & (v < 0.5),      # brown
        chromatic & (h >= 15.0) & (h < 45.0),                  # terracotta (v >= 0.5, implied)
        chromatic & (h >= 45.0) & (h < 70.0) & (v < 0.5),      # brown
        chromatic & (h >= 45.0) & (h < 70.0),                  # gold (v >= 0.5, implied)
        chromatic & (h >= 70.0) & (h < 170.0),                 # green
        chromatic & (h >= 170.0) & (h < 200.0),                # teal
        chromatic & (h >= 200.0) & (h < 260.0),                # blue
        chromatic & (h >= 260.0) & (h < 300.0),                # purple
    ]
    choices = [
        _COLOR_BUCKET_IDX["white"], _COLOR_BUCKET_IDX["light grey"], _COLOR_BUCKET_IDX["grey"],
        _COLOR_BUCKET_IDX["charcoal"], _COLOR_BUCKET_IDX["black"],
        _COLOR_BUCKET_IDX["cream"], _COLOR_BUCKET_IDX["beige"],
        _COLOR_BUCKET_IDX["red"], _COLOR_BUCKET_IDX["brown"], _COLOR_BUCKET_IDX["terracotta"],
        _COLOR_BUCKET_IDX["brown"], _COLOR_BUCKET_IDX["gold"],
        _COLOR_BUCKET_IDX["green"], _COLOR_BUCKET_IDX["teal"], _COLOR_BUCKET_IDX["blue"], _COLOR_BUCKET_IDX["purple"],
    ]
    # Remaining chromatic hues (300°-345°) -> pink, same as Rust's final `_ => "pink"`.
    return np.select(conditions, choices, default=_COLOR_BUCKET_IDX["pink"])


def color_histogram(img_rgb):
    """Gray-world white balance -> HSV bucket vote -> L2-normalised COLOR_DIM
    histogram, computed from an already-decoded RGB array. See module notes
    above for why this must track `color::histogram` bucket-for-bucket."""
    h0, w0 = img_rgb.shape[:2]
    scale = _HIST_MAX / max(h0, w0)
    new_w, new_h = max(1, round(w0 * scale)), max(1, round(h0 * scale))
    if (new_w, new_h) != (w0, h0):
        interp = cv2.INTER_AREA if scale < 1 else cv2.INTER_LINEAR
        small = cv2.resize(img_rgb, (new_w, new_h), interpolation=interp)
    else:
        small = img_rgb
    pixels = small.reshape(-1, 3).astype(np.float64)

    # ── Gray-world white balance ────────────────────────────────────────
    means = pixels.mean(axis=0)
    gray = means.mean()
    k = np.where(means > 1.0, gray / np.where(means > 1.0, means, 1.0), 1.0)
    balanced = np.clip(np.round(pixels * k), 0, 255)

    h, s, v = _rgb_to_hsv_vec(balanced / 255.0)
    bucket = _classify_vec(h, s, v)

    hist = np.bincount(bucket.astype(np.int64), minlength=COLOR_DIM)[:COLOR_DIM].astype(np.float64)
    norm = np.linalg.norm(hist)
    if norm > 0:
        hist = hist / norm
    return hist.astype(np.float32).tolist()


_SAMPLE_MAX = 48  # matches Rust's SAMPLE_MAX — long edge used for the dominant-colour vote


def dominant_colors(img_rgb):
    """1-2 dominant bucket *names* (primary first) for the Browse colour tag —
    e.g. ["beige"] or ["grey", "white"]. Port of the Rust `color::dominant`.

    Deliberately does NOT white-balance, unlike `color_histogram` above. The
    histogram feeds colour *similarity*, where gray-world correction is what
    makes one design under warm vs neutral light compare equal. A colour *tag*
    wants the opposite: it must report the chroma actually present. Measured on
    200 real tiles, reusing the white-balanced histogram here agreed with this
    function's top bucket only 66% of the time, and failed in one direction —
    blue/terracotta/cream all flattened toward grey, exactly the tint a tag is
    supposed to name. Two votes over the same decoded array, ~1 ms each, is the
    cheap way to have both.
    """
    h0, w0 = img_rgb.shape[:2]
    scale = _SAMPLE_MAX / max(h0, w0)
    new_w, new_h = max(1, round(w0 * scale)), max(1, round(h0 * scale))
    if (new_w, new_h) != (w0, h0):
        interp = cv2.INTER_AREA if scale < 1 else cv2.INTER_LINEAR
        small = cv2.resize(img_rgb, (new_w, new_h), interpolation=interp)
    else:
        small = img_rgb

    pixels = small.reshape(-1, 3).astype(np.float64) / 255.0
    h, s, v = _rgb_to_hsv_vec(pixels)
    counts = np.bincount(_classify_vec(h, s, v).astype(np.int64), minlength=COLOR_DIM)[:COLOR_DIM]

    total = int(counts.sum())
    if total == 0:
        return []
    order = np.argsort(counts)[::-1]
    out = [COLOR_BUCKETS[int(order[0])]]
    # Second colour only if it's a meaningful share (>= 25%) — captures e.g. a
    # grey tile with strong white veining. Same rule as the Rust original.
    if len(order) > 1 and int(counts[order[1]]) * 100 >= total * 25:
        out.append(COLOR_BUCKETS[int(order[1])])
    return out


@dataclass
class Descriptor:
    rose: list
    gram: list  # list of gram vectors, one per zoom level
    color: list  # COLOR_DIM-length, see color_histogram()
    dominant: list  # 1-2 bucket names for the Browse colour tag, see dominant_colors()


def describe(path, max_dim=512):
    """Full stage-1 descriptor for one image, or None if it can't be read."""
    t0 = time.perf_counter()
    img = load_image_rgb(path, max_dim=max_dim)
    t_decode = time.perf_counter()
    if img is None:
        log.debug("[timing] describe path=%s decode_ms=%.2f result=decode-failed",
                   os.path.basename(path), (t_decode - t0) * 1000)
        return None

    rose = gabor_rose(img)
    t_gabor = time.perf_counter()
    gram = gram_descriptor(img)
    t_gram = time.perf_counter()
    color = color_histogram(img)
    dominant = dominant_colors(img)
    t_color = time.perf_counter()

    log.debug(
        "[timing] describe path=%s dim=%dx%d decode_ms=%.2f gabor_ms=%.2f gram_ms=%.2f color_ms=%.2f total_ms=%.2f",
        os.path.basename(path), img.shape[1], img.shape[0],
        (t_decode - t0) * 1000, (t_gabor - t_decode) * 1000, (t_gram - t_gabor) * 1000,
        (t_color - t_gram) * 1000, (t_color - t0) * 1000,
    )
    return Descriptor(rose=rose.tolist(), gram=[g.tolist() for g in gram], color=color, dominant=dominant)


def prepare_query(query_path, max_dim=800):
    """Decode + SIFT the query once. `verify_one()` below reuses the result
    across a whole batch of candidates for the same query — the query-side
    SIFT pass is identical every time, so this way it only ever runs once
    per batch, not once per candidate. Returns `None` on decode failure.

    Deliberately sequential (called once, before any candidate work starts)
    — see `verify_one` for why the per-candidate work runs on its own SIFT
    instance instead of sharing this one."""
    q = load_image_rgb(query_path, max_dim=max_dim)
    if q is None:
        return None
    qg = cv2.cvtColor(q, cv2.COLOR_RGB2GRAY)
    kp1, des1 = _get_sift().detectAndCompute(qg, None)
    return {"path": query_path, "qg": qg, "kp": kp1, "des": des1}


def verify_one(query_state, candidate_path, ratio=0.75, ransac_thresh=5.0, min_inliers=12):
    """SIFT + RANSAC: does the query behind `query_state` (see
    `prepare_query`) actually appear inside `candidate_path`?

    `server._run` calls this from a thread pool — one candidate's decode +
    SIFT + RANSAC was ~350-450ms measured against the real 316-file library,
    so a 200-candidate shortlist run serially (the original design) took
    70-90s and blew straight through every timeout in the stack, silently
    degrading every result to "unverified". SIFT/RANSAC are OpenCV C++ calls
    that release the GIL, so threads scale close to linearly with cores here.
    Each call gets its own `cv2.SIFT_create()` rather than sharing the
    module-level instance `_get_sift()` returns, because cv2 algorithm
    objects aren't safe to invoke from multiple threads concurrently — only
    `query_state` (read-only past `prepare_query`) is shared.
    """
    t0 = time.perf_counter()
    if query_state is None:
        return {"matched": False, "reason": "decode-failed"}
    qg, kp1, des1 = query_state["qg"], query_state["kp"], query_state["des"]
    if des1 is None or len(kp1) < 8:
        return {"matched": False, "reason": "decode-failed"}

    c = load_image_rgb(candidate_path, max_dim=1600)
    t_decode = time.perf_counter()
    if c is None:
        return {"matched": False, "reason": "decode-failed"}
    cg = cv2.cvtColor(c, cv2.COLOR_RGB2GRAY)
    kp2, des2 = _new_sift().detectAndCompute(cg, None)
    t_cand_sift = time.perf_counter()
    if des2 is None or len(kp2) < 8:
        return {"matched": False, "reason": "too-few-keypoints"}

    bf = cv2.BFMatcher(cv2.NORM_L2)
    matches = bf.knnMatch(des1, des2, k=2)
    good = [m for m, n in matches if len(matches) and m.distance < ratio * n.distance]
    t_match = time.perf_counter()

    def log_timing(reason, **extra):
        log.debug(
            "[timing] verify query=%s candidate=%s decode_ms=%.2f "
            "cand_sift_ms=%.2f match_ms=%.2f ransac_ms=%.2f total_ms=%.2f reason=%s",
            os.path.basename(query_state["path"]), os.path.basename(candidate_path),
            (t_decode - t0) * 1000, (t_cand_sift - t_decode) * 1000,
            (t_match - t_cand_sift) * 1000, (time.perf_counter() - t_match) * 1000,
            (time.perf_counter() - t0) * 1000, reason,
        )

    if len(good) < 4:
        log_timing("too-few-matches")
        return {"matched": False, "reason": "too-few-matches", "good_matches": len(good)}

    src = np.float32([kp1[m.queryIdx].pt for m in good]).reshape(-1, 1, 2)
    dst = np.float32([kp2[m.trainIdx].pt for m in good]).reshape(-1, 1, 2)
    H, mask = cv2.findHomography(src, dst, cv2.RANSAC, ransac_thresh)
    if H is None:
        log_timing("no-homography")
        return {"matched": False, "reason": "no-homography", "good_matches": len(good)}

    inliers = int(mask.sum())
    inlier_ratio = inliers / len(good)
    matched = inliers >= min_inliers and inlier_ratio >= 0.35
    result = {"matched": matched, "good_matches": len(good), "inliers": inliers,
              "inlier_ratio": round(inlier_ratio, 3)}
    if matched:
        h, w = qg.shape
        corners = np.float32([[0, 0], [w, 0], [w, h], [0, h]]).reshape(-1, 1, 2)
        proj = cv2.perspectiveTransform(corners, H)
        result["box"] = proj.reshape(-1, 2).tolist()
        result["candidate_size"] = [c.shape[1], c.shape[0]]
        sx, sy = float(np.hypot(H[0, 0], H[1, 0])), float(np.hypot(H[0, 1], H[1, 1]))
        result["scale"] = round((sx + sy) / 2, 3)
    log_timing("matched" if matched else "low-confidence")
    return result
