# Pictoria sidecar

Local-only HTTP service (127.0.0.1) that Rust spawns hidden and keeps alive
for the app's session. Does the two things Rust doesn't have libraries for —
Gabor/Gram-matrix descriptors and SIFT/RANSAC verification — and nothing
else. Rust owns every read/write to `meta.db` and `vectors.bin`.

## Run (dev)

```bash
pip install -r requirements.txt
python server.py
```

First run downloads mobilenet_v2 weights (~14MB) via torchvision.

## API

- `GET /health` → `{"ready": bool}`
- `POST /describe` → `{"paths": [...], "priority": "search"|"index"}` →
  `{"results": [{"path", "rose", "gram"} | {"path", "error"}]}`
- `POST /verify` → `{"query_path": "...", "candidate_paths": [...], "priority": "search"|"index"}` →
  `{"results": [{"path", "matched", "inliers", "good_matches", "inlier_ratio",
  "box"?, "candidate_size"?, "scale"?, "reason"?}]}`

`priority: "search"` always runs before any queued `"index"` work — see
the comment on `_worker()` in `server.py`.

## Packaging

`.github/workflows/release.yml` freezes this with PyInstaller
(`--onefile --name pictoria-sidecar`) before the Tauri build step, on every
platform in the release matrix. Output lands at `sidecar/dist/pictoria-sidecar`
(`.exe` on Windows), which `UI/src-tauri/tauri.windows.conf.json` /
`tauri.macos.conf.json` map into the app bundle at `bin/pictoria-sidecar` —
`core::config::sidecar_bin_path()` looks for it there at runtime.

This hasn't been verified against a real CI run yet — first release build
after this change is worth watching closely. Local dev: if
`sidecar/dist/pictoria-sidecar.exe` doesn't exist, `core::sidecar::spawn()`
falls back to running `python server.py` directly (debug builds only), so
`cargo tauri dev` works without freezing anything first — but a plain
`cargo build`/`cargo check` still needs *some* file at that path (even an
empty placeholder) because Tauri's build script validates declared bundle
resources eagerly, before the app itself ever runs.
