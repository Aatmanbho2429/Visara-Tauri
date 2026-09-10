"""Descriptor computation and geometric verification for the Pictoria sidecar.

Two jobs only, matching the HTTP API: DESCRIBE (DINO embedding + Gabor +
Gram-matrix) and VERIFY (SIFT + RANSAC, for proving an actual embedded crop).
Everything else — storage, ranking, caching — lives in Rust now.

The DINO embedding is what selects the near family a search verifies against;
rose/gram are still computed and stored alongside it. Unlike the bundled
mobilenet, the DINO model ships encrypted and is only decryptable with a
licence key Supabase hands the Rust side at token-validate time, which Rust
then pushes to `POST /model` — so it loads after login, not at startup.
"""
from __future__ import annotations

import logging
import os
import sys
import threading
import time
from dataclasses import dataclass

import cv2
import numpy as np
import onnxruntime as ort
import torch
import torchvision
from cryptography.fernet import Fernet, InvalidToken

# Child of the "sidecar" logger configured in server.py — propagates to the
# same StreamHandler + FileHandler (sidecar/logs/sidecar.log), so these lines
# interleave with everything else on one timeline.
log = logging.getLogger("sidecar.pipeline")

# `server._run` already puts each verify candidate on its own pool thread
# (up to `_VERIFY_WORKERS`, typically 8) — without this, OpenCV additionally
# spins up its own internal `parallel_for_` pool *per call*, so 8 workers each
# drive a full pool against 4 physical cores. Confirmed by grep: nothing in
# this codebase called `setNumThreads`/`OPENCV_NUM_THREADS` before this line.
# One thread per call is correct here; the outer ThreadPoolExecutor is what
# provides the actual parallelism. See SEARCH-LATENCY-PLAN.md Phase 2a.
cv2.setNumThreads(1)

_DEVICE = "cpu"
_LAYER_IDX = 4  # mobilenet_v2 block deep enough for texture, shallow enough to stay local
_GABOR_ORIENTS = 8
_GABOR_FREQS = (0.08, 0.15, 0.25)
_GRAM_ZOOM_SCALES = (0.75, 1.0, 1.4)  # keeps ranking robust to unrelated images being different pixel scales

_model = None
_mean = None
_std = None
_sift = None

# ── DINO embedding model (encrypted, licence-gated) ──────────────────────
# Loaded by `load_embed_model()` when Rust posts the key to /model, never at
# import or worker startup.  `_embed_lock` guards the load/reset transition;
# `InferenceSession.run()` itself is thread-safe, so describe jobs read
# `_embed_sess` without holding it.
_embed_sess = None
_embed_input_name = None
_embed_output_name = None
_embed_dim = None
_embed_lock = threading.Lock()

# Side length of the square crop fed to both descriptors. Shared, so the DINO
# embedding and the gram matrix see byte-identical crops.
_EMBED_INPUT_SIZE = 224

# Expected width of one zoom level's embedding. Must equal Rust's
# `config::EMBED_DIM` — the vector store's entry stride is computed from it,
# so a mismatch is refused at load time rather than written to disk.
EMBED_DIM = 1536

# ImageNet normalisation, matching what the model was exported with (and what
# `load_model()` reads out of the torchvision weights metadata for gram).
# Spelled out as plain numpy here so the ONNX path carries no torch dependency.
_IMAGENET_MEAN = np.array([0.485, 0.456, 0.406], dtype=np.float32).reshape(1, 3, 1, 1)
_IMAGENET_STD = np.array([0.229, 0.224, 0.225], dtype=np.float32).reshape(1, 3, 1, 1)

# Encrypted DINO checkpoint. The `clip_vitb32` name is historical — the file
# holds DINO weights, not CLIP — and is kept deliberately: it is the name the
# published artefact and the key issued against it already use.
MODEL_ENC = "clip_vitb32.onnx.enc"


# Checkpoint filename as published by PyTorch. The `b0353104` suffix is the
# leading hash of its contents, which is how torch validates a download.
MOBILENET_CKPT = "mobilenet_v2-b0353104.pth"


def bundled_weights_path():
    """Where the frozen build keeps the checkpoint.

    `sys._MEIPASS` is the directory PyInstaller unpacks a --onefile build into
    at launch; outside a frozen build it falls back to this source tree, so a
    developer who drops the file in `sidecar/weights/` gets the same path.
    """
    base = getattr(sys, "_MEIPASS", None) or os.path.dirname(os.path.abspath(__file__))
    return os.path.join(base, "weights", MOBILENET_CKPT)


def bundled_model_path():
    """Where the encrypted DINO checkpoint lives.

    Same resolution rule as `bundled_weights_path()`: `sys._MEIPASS` in a
    frozen build (the file is listed in `pictoria-sidecar.spec`'s `datas`),
    the source tree otherwise — which is `sidecar/clip_vitb32.onnx.enc` for a
    developer checkout. Rust never sends a path, only the key, so this is the
    single place the location is decided.
    """
    base = getattr(sys, "_MEIPASS", None) or os.path.dirname(os.path.abspath(__file__))
    return os.path.join(base, MODEL_ENC)


def embed_model_ready():
    return _embed_sess is not None


def embed_model_dim():
    return _embed_dim


def reset_embed_model():
    """Drop the decrypted session — logout, or a lapsed subscription.

    The plaintext model only ever exists inside this process; clearing the
    session is what makes a licence check actually bite, since without it the
    model would stay usable for the rest of the app's session.
    """
    global _embed_sess, _embed_input_name, _embed_output_name, _embed_dim
    with _embed_lock:
        if _embed_sess is None:
            return
        _embed_sess = None
        _embed_input_name = None
        _embed_output_name = None
        _embed_dim = None
    log.info("DINO embed model unloaded")


def _fernet_decrypt(key_b64: str, token: bytes) -> bytes:
    """Fernet: URL_SAFE_BASE64(0x80 || ts(8) || IV(16) || ct || HMAC(32)).

    Same scheme the Rust `embedder::fernet_decrypt` used before this moved
    into the sidecar, so a key issued against the published artefact keeps
    working unchanged. `Fernet` verifies the HMAC before decrypting, so a
    wrong key raises rather than yielding garbage bytes for ORT to choke on.
    """
    try:
        return Fernet(key_b64.strip()).decrypt(token)
    except (InvalidToken, ValueError, TypeError) as e:
        raise ValueError(f"model decryption failed ({type(e).__name__})") from e


def load_embed_model(key_b64: str):
    """Decrypt and compile the DINO ONNX model. Returns its embedding width.

    Idempotent: a second call while a session is live is a no-op returning the
    live dimension, because Rust re-pushes the key on every token-validate and
    a rebuild would cost seconds of decrypt + compile for nothing.

    The output width is measured with one throwaway forward pass rather than
    read off the graph metadata, which is frequently a symbolic dim ('batch',
    'features') that tells us nothing. Rust refuses a mismatch against its own
    `EMBED_DIM` — the vector store's entry stride depends on it, so guessing
    here would corrupt `vectors.bin` rather than fail.
    """
    global _embed_sess, _embed_input_name, _embed_output_name, _embed_dim

    with _embed_lock:
        if _embed_sess is not None:
            return _embed_dim

        path = bundled_model_path()
        if not os.path.exists(path):
            raise FileNotFoundError(f"encrypted model not found: {path}")

        t0 = time.perf_counter()
        with open(path, "rb") as f:
            token = f.read()
        model_bytes = _fernet_decrypt(key_b64, token)
        t_decrypt = time.perf_counter()

        opts = ort.SessionOptions()
        opts.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
        # The DINO forward pass always runs on the single `_worker` thread
        # (server.py's one-job-at-a-time model), so ORT spinning up its own
        # multi-thread intra-op pool only contends with Rust's rayon pool
        # (`config::NUM_WORKERS = 8`) for no benefit — the same contention
        # `core/search_gate.rs` already exists to mitigate on the Rust side.
        # See SEARCH-LATENCY-PLAN.md Phase 2a.
        opts.intra_op_num_threads = 1
        # CoreML where it exists (Neural Engine on Apple Silicon, Metal on
        # Intel), CPU everywhere else. Listing CPU explicitly as the tail
        # provider is what makes an unsupported/unavailable CoreML fall back
        # instead of raising.
        providers = (["CoreMLExecutionProvider", "CPUExecutionProvider"]
                     if sys.platform == "darwin" else ["CPUExecutionProvider"])
        try:
            sess = ort.InferenceSession(model_bytes, sess_options=opts, providers=providers)
        except Exception:
            if providers[0] == "CPUExecutionProvider":
                raise
            log.warning("CoreML provider unavailable — falling back to CPU", exc_info=True)
            sess = ort.InferenceSession(model_bytes, sess_options=opts,
                                        providers=["CPUExecutionProvider"])
        t_compile = time.perf_counter()

        in_name = sess.get_inputs()[0].name
        out_name = sess.get_outputs()[0].name
        probe = np.zeros((1, 3, _EMBED_INPUT_SIZE, _EMBED_INPUT_SIZE), dtype=np.float32)
        out = sess.run([out_name], {in_name: probe})[0]
        dim = int(np.asarray(out).reshape(1, -1).shape[1])

        _embed_sess = sess
        _embed_input_name = in_name
        _embed_output_name = out_name
        _embed_dim = dim

    log.info(
        "DINO embed model ready: %s dim=%d providers=%s "
        "decrypt_ms=%.0f compile_ms=%.0f probe_ms=%.0f",
        os.path.basename(path), dim, sess.get_providers(),
        (t_decrypt - t0) * 1000, (t_compile - t_decrypt) * 1000,
        (time.perf_counter() - t_compile) * 1000,
    )
    if dim != EMBED_DIM:
        log.error("DINO embed dim %d != expected %d — Rust will refuse this model", dim, EMBED_DIM)
    return dim


def load_model():
    """Warm-load mobilenet_v2 once. Call at process startup, not per-request.

    The checkpoint is loaded from the app bundle, NOT fetched by torchvision.
    Passing `weights=IMAGENET1K_V1` makes torchvision download ~13.5MB from
    download.pytorch.org the first time it runs on a machine, which turned
    every fresh install into a silent network dependency: on a locked-down or
    offline machine the download stalls with no timeout, `_worker` never
    reaches `_ready.set()`, and the UI sits on "Model is loading..." forever
    while the HTTP server answers normally. Shipping the file removes the
    dependency entirely — verified to produce a bit-identical model (all 156
    params and 156 buffers equal, and identical descriptors), so bundling does
    not disturb any already-indexed vectors.

    The download path is kept only as a developer convenience for a source
    checkout with no weights staged; a frozen build always has the file.
    """
    global _model, _mean, _std
    if _model is None:
        weights = torchvision.models.MobileNet_V2_Weights.IMAGENET1K_V1
        ckpt = bundled_weights_path()

        if os.path.exists(ckpt):
            full = torchvision.models.mobilenet_v2(weights=None)
            full.load_state_dict(torch.load(ckpt, map_location=_DEVICE))
            log.info("model weights loaded from bundle: %s", ckpt)
        else:
            # Never expected in a packaged build — see the CI "Fetch model
            # weights" step, which stages the file before PyInstaller runs.
            log.warning(
                "bundled weights missing at %s — falling back to torchvision "
                "download (requires internet; packaged builds must not hit this)",
                ckpt,
            )
            full = torchvision.models.mobilenet_v2(weights=weights)

        features = full.features.to(_DEVICE).eval()
        _model = torch.nn.Sequential(*list(features.children())[:_LAYER_IDX])
        # Normalisation constants come from the weights *metadata*, which is
        # static — reading them does not trigger a download.
        _mean = torch.tensor(weights.transforms().mean).view(1, 3, 1, 1)
        _std = torch.tensor(weights.transforms().std).view(1, 3, 1, 1)
    return _model


def _new_sift():
    # nfeatures lowered from 4000 (SEARCH-LATENCY-PLAN.md Phase 2c) —
    # BFMatcher.knnMatch is brute force, O(|des1|*|des2|*128), so keypoint
    # count enters the dominant per-candidate cost quadratically. This
    # changes match sensitivity and MUST be validated against the known-good
    # verification set (21 colourway pairs verify, `BRASIL GREY P4.jpg` stays
    # rejected) before it ships — not yet done, see the plan's Phase 2
    # acceptance criteria.
    return cv2.SIFT_create(nfeatures=1500, contrastThreshold=0.01, edgeThreshold=20)


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


def zoom_crops(img_rgb, size=_EMBED_INPUT_SIZE, zoom_scales=_GRAM_ZOOM_SCALES):
    """Desaturated square crops of one image, one per zoom level.

    Shared by `gram_descriptor` and `embed_descriptor` so both descriptors are
    computed from byte-identical pixels, and the resize/crop work is done once
    per file instead of twice.

    Desaturated (grayscale, replicated to 3 channels) rather than the original
    RGB — ImageNet normalisation is very colour-sensitive, which used to bury
    same-design different-colourway files (e.g. a "small_yellow" variant of
    "small_blue") deep enough that they'd fall out of the verify set entirely,
    even though `gabor_rose` (already grayscale) recognised the shared
    structure fine. Keeping both descriptors grayscale-only makes retrieval
    genuinely structure-led, with colour staying display-only via
    `color_histogram` — see [[pictoria-search-architecture]].

    Resize is short-side-to-target then centre-crop; never pad. Padding gives
    every image of the same aspect ratio an identical band artefact which then
    dominates similarity — measured previously, a 1.98-aspect query returned
    twenty 1.98-aspect images regardless of design while the true parent sat at
    rank 237.

    The several zoom levels are what keep matching robust to two related images
    being at different pixel scales.
    """
    gray = cv2.cvtColor(img_rgb, cv2.COLOR_RGB2GRAY)
    img_struct = cv2.cvtColor(gray, cv2.COLOR_GRAY2RGB)
    h0, w0 = img_struct.shape[:2]
    crops = []
    for zoom in zoom_scales:
        target_short = max(8, int(round(size / zoom)))
        scale = target_short / min(h0, w0)
        rh, rw = max(size, int(round(h0 * scale))), max(size, int(round(w0 * scale)))
        resized = cv2.resize(img_struct, (rw, rh), interpolation=cv2.INTER_AREA if scale < 1 else cv2.INTER_CUBIC)
        y0, x0 = (rh - size) // 2, (rw - size) // 2
        crops.append(resized[y0:y0 + size, x0:x0 + size])
    return crops


@torch.no_grad()
def gram_descriptor(img_rgb=None, size=_EMBED_INPUT_SIZE, zoom_scales=_GRAM_ZOOM_SCALES, crops=None):
    """Channel-correlation texture-style descriptor, one vector per zoom level.

    Pass `crops` from `zoom_crops()` to reuse the crops the embedding already
    needed; `img_rgb` is the standalone path and just computes them itself.
    """
    if crops is None:
        crops = zoom_crops(img_rgb, size=size, zoom_scales=zoom_scales)
    model = load_model()
    grams = []
    for crop in crops:
        t = torch.from_numpy(crop).float().permute(2, 0, 1).unsqueeze(0) / 255.0
        t = (t - _mean) / _std
        feat = model(t)
        b, c, h, w = feat.shape
        f = feat.reshape(c, h * w)
        g = (f @ f.t()) / (c * h * w)
        grams.append(g.flatten().numpy())
    return grams


def embed_descriptor(img_rgb=None, size=_EMBED_INPUT_SIZE, zoom_scales=_GRAM_ZOOM_SCALES, crops=None):
    """DINO embedding per zoom level — the vector the near-family scan uses.

    One batched ONNX call for all zoom levels rather than one per level: the
    crops are the same shape by construction, and a single batch of 3 is
    materially cheaper than 3 batches of 1 on both CoreML and CPU.

    Each level is L2-normalised independently so the Rust side's cosine is a
    plain dot product and no zoom level can dominate on magnitude alone.

    Raises if the model isn't loaded — callers gate on `embed_model_ready()`
    (and Rust gates indexing/search on `/health`'s `embed_ready`), so reaching
    here without a session is a bug worth surfacing, not a silent empty result.
    """
    sess = _embed_sess
    if sess is None:
        raise RuntimeError("DINO embed model not loaded")
    if crops is None:
        crops = zoom_crops(img_rgb, size=size, zoom_scales=zoom_scales)

    batch = np.stack(crops).astype(np.float32).transpose(0, 3, 1, 2) / 255.0
    batch = (batch - _IMAGENET_MEAN) / _IMAGENET_STD
    out = sess.run([_embed_output_name], {_embed_input_name: np.ascontiguousarray(batch)})[0]
    out = np.asarray(out, dtype=np.float32).reshape(len(crops), -1)
    norms = np.linalg.norm(out, axis=1, keepdims=True)
    np.maximum(norms, 1e-9, out=norms)
    return out / norms


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


# ── Geometric plausibility of an accepted homography ─────────────────────
# A high inlier count alone does not mean the query really sits inside the
# candidate. `findHomography` fits 8 degrees of freedom, so on a self-similar
# surface (marble, wood grain, fabric weave) it can bend into a rotated,
# keystoned transform that happens to collect a couple of dozen of the
# ambiguous SIFT matches such surfaces produce, and pass `min_inliers` /
# `inlier_ratio` on merit. The resulting "match" is geometric nonsense:
# a real embedded crop lands as an upright, essentially unwarped rectangle.
#
# Thresholds measured against the full 174-distinct-image library with a
# grey-marble screenshot as the query. The 7 genuine matches vs the 1 false
# positive (BRASIL GREY P4.jpg) separated on every axis with room to spare:
#
#                       genuine (n=7)        false positive
#   contained           0.859 - 1.000        0.582
#   keystone            0.000 - 0.026        0.086
#   rotation off-axis   0.0 - 1.0 deg        21.5 deg
#
# Each bound below independently rejects that false positive, so a texture
# that defeats one still has to get past the other two.
_MIN_CONTAINED = 0.75   # fraction of the projected quad that must land in-frame
_MAX_KEYSTONE = 0.05    # perspective warp across the candidate's own span
_MAX_ROT_OFF_DEG = 10.0  # deviation from the nearest right angle


def _placement_geometry(H, q_shape, c_shape):
    """Measure the projected query quad. Returns (metrics, quad)."""
    qh, qw = q_shape
    ch, cw = c_shape
    corners = np.float32([[0, 0], [qw, 0], [qw, qh], [0, qh]]).reshape(-1, 1, 2)
    quad = cv2.perspectiveTransform(corners, H).reshape(-1, 2)

    x, y = quad[:, 0], quad[:, 1]
    area = float(0.5 * np.sum(x * np.roll(y, -1) - np.roll(x, -1) * y))

    # How much of the claimed region actually falls inside the candidate.
    # "Found inside" is not a similarity judgement — it is a containment
    # claim, so a quad hanging half-way off the frame refutes itself. The
    # Rust side clamps this quad to the frame before drawing the highlight
    # box (`search::quad_to_fraction_box`), which is right for drawing but
    # means an out-of-bounds match still *looks* plausible in the UI — so
    # it has to be caught here, before the result is reported as verified.
    if abs(area) > 1.0:
        frame = np.float32([[0, 0], [cw, 0], [cw, ch], [0, ch]])
        inter, _ = cv2.intersectConvexConvex(quad.astype(np.float32), frame)
        contained = inter / abs(area)
    else:
        contained = 0.0

    keystone = max(abs(H[2, 0]) * cw, abs(H[2, 1]) * ch)

    edge = quad[1] - quad[0]
    rot = abs(np.degrees(np.arctan2(edge[1], edge[0]))) % 90.0
    rot_off = min(rot, 90.0 - rot)

    return {"contained": float(contained), "keystone": float(keystone),
            "rot_off": float(rot_off), "area": area}, quad


def _implausible_placement(geo):
    """Reason string if this placement can't be a real embedded crop, else None."""
    if geo["area"] <= 1.0:
        return "degenerate-homography"
    if geo["contained"] < _MIN_CONTAINED:
        return "match-outside-frame"
    if geo["keystone"] > _MAX_KEYSTONE:
        return "implausible-warp"
    if geo["rot_off"] > _MAX_ROT_OFF_DEG:
        return "implausible-rotation"
    return None


@dataclass
class Descriptor:
    embed: list  # DINO embedding, one EMBED_DIM vector per zoom level
    rose: list
    gram: list  # list of gram vectors, one per zoom level
    color: list  # COLOR_DIM-length, see color_histogram()
    dominant: list  # 1-2 bucket names for the Browse colour tag, see dominant_colors()


def describe(path, max_dim=512):
    """Full descriptor for one image, or None if it can't be read.

    The zoom crops are computed once and handed to both the DINO embedding and
    the gram matrix — they are the same pixels, and doing the resize/crop twice
    was pure waste.
    """
    t0 = time.perf_counter()
    img = load_image_rgb(path, max_dim=max_dim)
    t_decode = time.perf_counter()
    if img is None:
        log.debug("[timing] describe path=%s decode_ms=%.2f result=decode-failed",
                   os.path.basename(path), (t_decode - t0) * 1000)
        return None

    crops = zoom_crops(img)
    t_crops = time.perf_counter()
    embed = embed_descriptor(crops=crops)
    t_embed = time.perf_counter()
    rose = gabor_rose(img)
    t_gabor = time.perf_counter()
    gram = gram_descriptor(crops=crops)
    t_gram = time.perf_counter()
    color = color_histogram(img)
    dominant = dominant_colors(img)
    t_color = time.perf_counter()

    log.debug(
        "[timing] describe path=%s dim=%dx%d decode_ms=%.2f crops_ms=%.2f embed_ms=%.2f "
        "gabor_ms=%.2f gram_ms=%.2f color_ms=%.2f total_ms=%.2f",
        os.path.basename(path), img.shape[1], img.shape[0],
        (t_decode - t0) * 1000, (t_crops - t_decode) * 1000, (t_embed - t_crops) * 1000,
        (t_gabor - t_embed) * 1000, (t_gram - t_gabor) * 1000,
        (t_color - t_gram) * 1000, (t_color - t0) * 1000,
    )
    return Descriptor(embed=[e.tolist() for e in embed], rose=rose.tolist(),
                      gram=[g.tolist() for g in gram], color=color, dominant=dominant)


def prepare_query(query_path, max_dim=800, mirror=False):
    """Decode + SIFT the query once. `verify_one()` below reuses the result
    across a whole batch of candidates for the same query — the query-side
    SIFT pass is identical every time, so this way it only ever runs once
    per batch, not once per candidate. Returns `None` on decode failure.

    Deliberately sequential (called once, before any candidate work starts)
    — see `verify_one` for why the per-candidate work runs on its own SIFT
    instance instead of sharing this one.

    `mirror=True` horizontally flips the query before SIFT, which is how a
    mirrored match is found. It cannot be found any other way: SIFT
    descriptors are not mirror-invariant (the 4x4 gradient grid reflects
    into a different 128-vector), and `estimateAffinePartial2D` below fits
    `[[a, -b, tx], [b, a, ty]]`, whose determinant `a^2 + b^2` is strictly
    positive — the model cannot express a reflection at all. Flipping the
    query converts the problem back into a plain rotation + uniform scale +
    translation, which both SIFT and that model handle normally.

    A horizontal flip is sufficient for every reflection: any other one is
    this flip composed with a rotation, and rotation is already covered
    (SIFT is rotation-invariant, the affine model includes rotation, and the
    geometry gate measures `rot_off` against the *nearest* right angle).

    The flip preserves the image's dimensions, so `qg.shape` — which
    `_placement_geometry` uses to project the query's corners — is unchanged
    and the resulting quad is still the correct region *of the candidate*.
    """
    q = load_image_rgb(query_path, max_dim=max_dim)
    if q is None:
        return None
    qg = cv2.cvtColor(q, cv2.COLOR_RGB2GRAY)
    if mirror:
        qg = cv2.flip(qg, 1)
    kp1, des1 = _get_sift().detectAndCompute(qg, None)
    return {"path": query_path, "qg": qg, "kp": kp1, "des": des1, "mirror": mirror}


def verify_one(query_state, candidate_path, ratio=0.75, ransac_thresh=5.0, min_inliers=10):
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

    # Lowered from 1600 (SEARCH-LATENCY-PLAN.md Phase 2c) — SIFT cost scales
    # with pixel count, and the query itself already decodes at 800
    # (`prepare_query`). Also not free: validate against the known-good set
    # before shipping, same caveat as `_new_sift`'s nfeatures change above.
    c = load_image_rgb(candidate_path, max_dim=1024)
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

    # A crop of flat art is a SIMILARITY transform — rotate, uniform scale,
    # translate (4 DOF). It is never a projective warp, so fitting the 8-DOF
    # homography `findHomography` gives RANSAC four degrees of freedom it can
    # only misuse. That cut both ways, measured on the real library:
    #
    #  - False positives: on self-similar stone, the spare DOF let RANSAC bend
    #    a keystoned, rotated transform onto a couple of dozen ambiguous
    #    matches (see `_MIN_CONTAINED`).
    #  - False negatives: on a weak-but-real match (a recoloured crop, ~16
    #    good matches) the fit is under-constrained, so the same spare DOF
    #    bent a genuine match into a warp the geometry gate then rejected.
    #    A grey slab that truly contains the query was being dropped this way.
    #
    # The constrained model cannot express either distortion, so both go away
    # without touching the thresholds: 21/21 known colourway pairs still
    # verify, the known false positive stays rejected, and the true matches
    # that were being lost come back.
    M, mask = cv2.estimateAffinePartial2D(
        src, dst, method=cv2.RANSAC, ransacReprojThreshold=ransac_thresh,
        maxIters=5000, confidence=0.995,
    )
    if M is None or mask is None:
        log_timing("no-transform")
        return {"matched": False, "reason": "no-transform", "good_matches": len(good)}
    H = np.vstack([M, [0.0, 0.0, 1.0]]).astype(np.float64)

    inliers = int(mask.sum())
    inlier_ratio = inliers / len(good)
    # `min_inliers` is a floor against degenerate fits, not a confidence
    # measure — `inlier_ratio` is what actually separates signal from noise,
    # because it is scale-free. The absolute count is not: a recoloured crop
    # keeps only a fraction of the query's SIFT correspondences (measured:
    # 496 inliers between two greyscale-identical images collapsed to ~12
    # once one side was recoloured), so a fixed floor silently penalises
    # exactly the colourway matches this app exists to find. 12 sat right on
    # top of that cluster — a 2px border trim on the query was enough to move
    # a real match from 12 to 11 and drop it out of the verified tier.
    matched = inliers >= min_inliers and inlier_ratio >= 0.35
    result = {"matched": matched, "good_matches": len(good), "inliers": inliers,
              "inlier_ratio": round(inlier_ratio, 3),
              # Which orientation of the query produced this. Only meaningful
              # when `matched`; the caller reports it so the UI can say the
              # match is a mirror rather than silently presenting it as a
              # straight hit (book-matched tile pairs are a real case).
              "mirrored": bool(query_state.get("mirror", False))}

    reason = "matched" if matched else "low-confidence"
    if matched:
        # Counting votes is not enough — check the transform those votes
        # elected is one a real embedded crop could produce. See the
        # `_MIN_CONTAINED` block above for why and for the measured margins.
        geo, quad = _placement_geometry(H, qg.shape, cg.shape)
        rejected = _implausible_placement(geo)
        if rejected:
            log.debug(
                "[verify] rejected candidate=%s on geometry (%s): inliers=%d ratio=%.2f "
                "contained=%.3f keystone=%.3f rot_off=%.1f",
                os.path.basename(candidate_path), rejected, inliers, inlier_ratio,
                geo["contained"], geo["keystone"], geo["rot_off"],
            )
            matched = False
            result["matched"] = False
            result["reason"] = rejected
            reason = rejected
        else:
            result["box"] = quad.tolist()
            result["candidate_size"] = [c.shape[1], c.shape[0]]
            sx, sy = float(np.hypot(H[0, 0], H[1, 0])), float(np.hypot(H[0, 1], H[1, 1]))
            result["scale"] = round((sx + sy) / 2, 3)
    log_timing(reason)
    return result
