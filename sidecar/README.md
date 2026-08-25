# Pictoria sidecar

Local-only HTTP service (127.0.0.1) that Rust spawns hidden and keeps alive
for the app's session. Does the things Rust doesn't have libraries for — DINO
embeddings (ONNX), Gabor/Gram-matrix descriptors, and SIFT/RANSAC verification
— and nothing else. Rust owns every read/write to `meta.db` and `vectors.bin`.

## The two models

| | mobilenet_v2 (gram) | DINO (embeddings) |
|---|---|---|
| ships as | `weights/*.pth`, plaintext | `clip_vitb32.onnx.enc`, Fernet-encrypted |
| loads | worker startup | when Rust posts the key to `/model` |
| gates | `/health` `ready` | `/health` `embed_ready` |

The DINO model is the licensed one. Supabase returns an `onnx_key` at
token-validate; `services::auth` (Rust) hands it to `core::sidecar`, which
posts it here. The plaintext model exists only in this process's memory and is
dropped on logout or a lapsed subscription (`POST /model/reset`).

The `clip_vitb32` filename is historical — the file holds DINO weights, not
CLIP. It is kept because the published artefact and the key issued against it
already use that name.

## Run (dev)

```bash
pip install -r requirements.txt
python server.py
```

First run downloads mobilenet_v2 weights (~14MB) via torchvision. The DINO
model is read from `sidecar/clip_vitb32.onnx.enc` and needs a valid key posted
to `/model` before `/describe` will answer — without it every describe returns
`{"error": "embed-model-not-loaded"}` and Rust's `sidecar::is_ready()` stays
false, so indexing and search never start.

## API

- `GET /health` → `{"ready": bool, "error": str?, "embed_ready": bool, "embed_dim": int?}`
- `POST /model` → `{"key": "..."}` → `{"ok": true, "embed_dim": int}` or `{"error"}` (500).
  Blocks while decrypting + compiling (seconds). Idempotent — a second call
  with the model already up returns the live dimension without rebuilding.
- `POST /model/reset` → `{"ok": true}`. Unloads the model.
- `POST /describe` → `{"paths": [...], "priority": "search"|"index"}` →
  `{"results": [{"path", "embed", "rose", "gram", "color", "dominant"} | {"path", "error"}]}`.
  Returns `{"error": "embed-model-not-loaded"}` for the whole batch if `/model`
  has not been called.
- `POST /verify` → `{"query_path": "...", "candidate_paths": [...], "priority": "search"|"index"}` →
  `{"results": [{"path", "matched", "inliers", "good_matches", "inlier_ratio",
  "box"?, "candidate_size"?, "scale"?, "reason"?}]}`

`priority: "search"` always runs before any queued `"index"` work — see
the comment on `_worker()` in `server.py`.

## Packaging

Both models are bundled into the frozen binary via `--add-data` and resolved
at runtime through `sys._MEIPASS` (see `pipeline.bundled_weights_path` and
`bundled_model_path`). The mobilenet checkpoint is downloaded and hash-checked
in CI; the encrypted DINO model comes from the repo through Git LFS, and the
**Verify encrypted DINO model** step rejects an unexpanded LFS pointer — which
would otherwise be bundled silently and fail only on a user's machine.

`.github/workflows/release.yml` freezes this with PyInstaller
(`--onefile --name pictoria-sidecar`) before the Tauri build step, on every
platform in the release matrix, then does two things with the output:

1. **Stages it for `bundle.externalBin`** (declared in `tauri.conf.json` as
   `binaries/pictoria-sidecar`). Tauri resolves that per-platform as
   `binaries/pictoria-sidecar-<target-triple>[.exe]`, so CI copies
   `dist/pictoria-sidecar` to that name under `UI/src-tauri/binaries/`.
   Tauri strips the suffix again when it copies the binary into the bundle,
   landing it *next to the main executable* (`Contents/MacOS/` on macOS) —
   which is the first path `core::config::sidecar_bin_path()` probes.
2. **Signs it (macOS only)** with the Developer ID identity, `--options
   runtime`, and `UI/src-tauri/entitlements.plist`. This has to happen before
   tauri-action seals the `.app`, because the outer signature covers its
   contents.

Both steps matter for notarization: Apple requires every Mach-O inside the
bundle to carry a Developer ID signature *and* the hardened runtime, and
PyInstaller leaves its output ad-hoc signed with neither. The hardened runtime
in turn breaks a `--onefile` binary unless the entitlements are granted — see
the comment block in `entitlements.plist` for why each key is there. The
signing step self-checks and fails the job if the binary is still ad-hoc,
rather than letting it surface ~40 minutes later at notarization.

### Per-architecture builds

Rust cross-compiles from a `--target` flag; PyInstaller cannot — it freezes
whatever interpreter runs it. On the Apple Silicon runner that meant the
`x86_64-apple-darwin` leg produced an *arm64* sidecar, which an Intel Mac
cannot execute at all (Rosetta translates x86_64 → arm64, never the reverse).
The app would launch, `spawn()` would fail, and search would silently never
work.

Each matrix leg therefore carries a `py_arch`, and `actions/setup-python`
installs an interpreter of that architecture — x86_64 CPython running under
Rosetta for the Intel leg. PyInstaller then freezes the right architecture
because it is running on it, and a **Verify sidecar architecture** step asserts
`lipo -archs` matches the target, so a mismatch fails the job instead of
reaching users.

An x86_64 interpreter also makes pip resolve x86_64 wheels, and torch stopped
publishing those after 2.2.2 (torchvision 0.17.2). The Intel leg therefore uses
its own pinned `requirements-macos-x86_64.txt`; every other platform uses
`requirements.txt`. That pairing is verified to resolve for cp311/x86_64. It is
a frozen stack — no further torch updates exist for this platform — so if
maintaining it stops being worth it, dropping the Intel target is the
alternative.

### Local dev

`cargo build`/`cargo check` needs `UI/src-tauri/binaries/pictoria-sidecar-<your
host triple>` to exist — Tauri's build script validates declared external
binaries eagerly, before the app ever runs, and fails with `resource path ...
doesn't exist` otherwise. Stage it with:

```bash
cp sidecar/dist/pictoria-sidecar \
   "UI/src-tauri/binaries/pictoria-sidecar-$(rustc -vV | awk '/host:/{print $2}')"
```

That directory is gitignored (~200MB). At *runtime* in a debug build,
`core::sidecar::spawn()` still falls back to `python server.py` if no frozen
binary is found, so `cargo tauri dev` works without freezing first — the
staged file is only needed to get past the build script.
