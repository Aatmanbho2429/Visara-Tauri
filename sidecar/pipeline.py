"""Descriptor computation and geometric verification for the Pictoria sidecar.

Two jobs only, matching the HTTP API: DESCRIBE (Gabor + Gram-matrix, for
ranking) and VERIFY (SIFT + RANSAC, for proving an actual embedded crop).
Everything else — storage, ranking, caching — lives in Rust now.
"""
from __future__ import annotations

import os
from dataclasses import dataclass

import cv2
import numpy as np
import torch
import torchvision

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


def _get_sift():
    global _sift
    if _sift is None:
        _sift = cv2.SIFT_create(nfeatures=4000, contrastThreshold=0.01, edgeThreshold=20)
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
    ranking isn't thrown off by two related images being different pixel scales."""
    model = load_model()
    h0, w0 = img_rgb.shape[:2]
    grams = []
    for zoom in zoom_scales:
        target_short = max(8, int(round(size / zoom)))
        scale = target_short / min(h0, w0)
        rh, rw = max(size, int(round(h0 * scale))), max(size, int(round(w0 * scale)))
        resized = cv2.resize(img_rgb, (rw, rh), interpolation=cv2.INTER_AREA if scale < 1 else cv2.INTER_CUBIC)
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


@dataclass
class Descriptor:
    rose: list
    gram: list  # list of gram vectors, one per zoom level


def describe(path, max_dim=512):
    """Full stage-1 descriptor for one image, or None if it can't be read."""
    img = load_image_rgb(path, max_dim=max_dim)
    if img is None:
        return None
    return Descriptor(rose=gabor_rose(img).tolist(), gram=[g.tolist() for g in gram_descriptor(img)])


def verify(query_path, candidate_path, query_cache, ratio=0.75, ransac_thresh=5.0, min_inliers=12):
    """SIFT + RANSAC: does `query_path` actually appear inside `candidate_path`?

    `query_cache` is a plain dict the caller reuses across a whole batch of
    candidates for the same query, so the query's own SIFT pass — identical
    every time — only ever runs once per batch, not once per candidate.
    """
    sift = _get_sift()
    if query_cache.get("path") != query_path:
        q = load_image_rgb(query_path, max_dim=800)
        if q is None:
            return {"matched": False, "reason": "decode-failed"}
        qg = cv2.cvtColor(q, cv2.COLOR_RGB2GRAY)
        kp1, des1 = sift.detectAndCompute(qg, None)
        query_cache.update(path=query_path, qg=qg, kp=kp1, des=des1)
    qg, kp1, des1 = query_cache["qg"], query_cache["kp"], query_cache["des"]

    c = load_image_rgb(candidate_path, max_dim=1600)
    if c is None or des1 is None or len(kp1) < 8:
        return {"matched": False, "reason": "decode-failed"}
    cg = cv2.cvtColor(c, cv2.COLOR_RGB2GRAY)
    kp2, des2 = sift.detectAndCompute(cg, None)
    if des2 is None or len(kp2) < 8:
        return {"matched": False, "reason": "too-few-keypoints"}

    bf = cv2.BFMatcher(cv2.NORM_L2)
    matches = bf.knnMatch(des1, des2, k=2)
    good = [m for m, n in matches if len(matches) and m.distance < ratio * n.distance]
    if len(good) < 4:
        return {"matched": False, "reason": "too-few-matches", "good_matches": len(good)}

    src = np.float32([kp1[m.queryIdx].pt for m in good]).reshape(-1, 1, 2)
    dst = np.float32([kp2[m.trainIdx].pt for m in good]).reshape(-1, 1, 2)
    H, mask = cv2.findHomography(src, dst, cv2.RANSAC, ransac_thresh)
    if H is None:
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
    return result
