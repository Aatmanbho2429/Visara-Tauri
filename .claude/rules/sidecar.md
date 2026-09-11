---
paths:
  - "sidecar/**/*.py"
  - "UI/src-tauri/src/core/sidecar.rs"
---

# The sidecar contract (`core::sidecar` ↔ `sidecar/server.py`)

A local-only Flask service on 127.0.0.1, spawned hidden by Rust at startup and kept alive for the
session. It computes and returns data over HTTP and **never persists anything** — Rust owns every
read and write to `meta.db` and `vectors.bin`.

- A background thread respawns it on crash, bounded: after `MAX_CONSECUTIVE_CRASHES` it gives up
  and emits a `sidecar_crashed` event for the UI rather than looping forever.
- Two queues, one worker thread. `"search"` jobs always run before queued `"index"` jobs, so an
  interactive search never waits behind a folder index — indexing pauses and resumes around it.
- The model key lives in memory only, on both sides. Never written to disk, never logged. Dropped
  on logout or a lapsed subscription.

## Endpoints

| Endpoint | Notes |
|---|---|
| `GET /health` | Two readiness flags: `ready` (mobilenet/gram, loads at worker startup) and `embed_ready` (DINO, gated on the key) |
| `POST /model` | Push the decryption key. Blocks while decrypting and compiling — budget generously, seconds to minutes cold |
| `POST /model/reset` | Drop the key |
| `POST /describe` | Batch → embed/rose/gram/color descriptors |
| `POST /verify` | SIFT/RANSAC verification, parallelized server-side across an 8-way pool |

The DINO model ships encrypted (`clip_vitb32.onnx.enc`, Git LFS) and only loads once Rust posts a
licence-derived key after a successful login. Without it `/describe` returns
`{"error": "embed-model-not-loaded"}` and indexing/search never start.

## Invariant

`config::EMBED_DIM` must match the model's real output width. `pipeline.py::load_embed_model`
measures it and reports it through `/health`; `core::sidecar` refuses a mismatch rather than let a
wrong stride reach `vectors.bin`. `EMBED_ZOOM_LEVELS` mirrors `_GRAM_ZOOM_SCALES` here — the crops
are literally shared. Changing how any descriptor is produced also means bumping
`core::migrate::EMBED_SCHEMA_VERSION`.

## Dev loop

```bash
pip install -r requirements.txt
python server.py               # first run downloads mobilenet_v2 weights (~14MB)
```

Freezing into a binary — only needed to test the frozen path, not for normal dev:

```bash
pyinstaller --onefile --name pictoria-sidecar \
  --add-data "weights/mobilenet_v2-b0353104.pth<SEP>weights" \
  --add-data "clip_vitb32.onnx.enc<SEP>." \
  server.py   # <SEP> is ';' on Windows, ':' elsewhere
```

Releases cross-freeze per-platform in `.github/workflows/release.yml` on `v*` tags, including an
x86_64-under-Rosetta leg for Intel macOS — PyInstaller freezes whatever interpreter runs it, not a
`--target`. macOS code-signs the sidecar before `tauri-action` seals the `.app`.
