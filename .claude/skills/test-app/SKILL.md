---
name: test-app
description: Run the Pictoria end-to-end test suite against the running app — search verification regressions (SIFT/RANSAC), sidecar API contract, vector-store integrity, stage-1 recall, and search latency parsed from the app's own logs. Use when asked to test the app, check for regressions, validate a search/sidecar/vector-store change, calibrate NEAR_FAMILY_MIN_SIM, or confirm search is still finding the right matches.
argument-hint: [suite ...]
---

# Test the running Pictoria app

Exercises the **live app** — its sidecar, its SQLite store, its `vectors.bin`, and its
logs — rather than mocks. Most of what breaks in this app breaks in the interaction
between those pieces, which unit tests cannot reach.

## Run it

The app must be running (`npx tauri dev`) **and logged in** — the DINO model only loads
after auth pushes the licence key, and `/describe` returns `embed-model-not-loaded`
until then.

```bash
sidecar/.venv/bin/python .claude/skills/test-app/scripts/pictoria_test.py
```

Use the sidecar's own venv interpreter — it already has PIL/numpy, and the harness uses
only stdlib for HTTP so nothing else needs installing. Run one or more suites by name,
e.g. `... pictoria_test.py verify latency`. Exit code is non-zero if anything failed.

| Suite | What it proves |
|---|---|
| `preflight` | App up, sidecar healthy, both models loaded, `embed_dim` agrees with `config::EMBED_DIM`, library indexed |
| `describe` | `/describe` contract: descriptor widths match `config.rs`, embeddings are unit-normalised per zoom, output is deterministic, an undecodable path fails softly instead of poisoning the batch |
| `verify` | The SIFT/RANSAC regression set — the canonical yellow-tile case, the known false positive, and every colourway pair |
| `store` | `vectors.bin` header/stride/version, no duplicate or orphaned ids against `meta.db`, and every stored embedding zoom level is unit-normalised |
| `nearfamily` | Stage-1 recall, computed independently of the app: the known matches must clear the cosine floor, plus a cosine histogram for calibrating `NEAR_FAMILY_MIN_SIM` |
| `latency` | Parses `[timing] search TOTAL` and `[calibration]` out of `Pictoria.log` — per-stage costs, verify-pool bounds, whether the budget was exhausted |
| `unit` | `cargo test` + `npm run build`. Not in the default run; add it explicitly |

## How to read the results

**`matched` and `inlier_ratio` are the pass/fail contract. Absolute inlier counts are
not.** Recolouring destroys most SIFT correspondences — measured on this library, a pair
that scores 496 inliers greyscale-identical survives on ~12 once one side is recoloured
— so an absolute floor systematically penalises exactly the colourway matches the app
exists to find. The harness therefore reports inliers as a **margin over
`pipeline.verify_one`'s own `min_inliers=10`** and flags a thin margin `[fragile]`
without failing. A `[fragile]` row is the early warning: the match still works, but it is
close to vanishing.

`INFO` rows are measurements, not assertions — the cosine histogram and the latency table
are there to be read, not to pass.

## Facts the suite depends on

- **The query is trimmed before use.** `search_service.rs::trim_uniform_border` crops a
  uniform margin off the query and re-encodes it; that crop alone once moved a canonical
  match from 12 inliers to 11. `trim_like_app()` in the harness is a port of that
  function. Testing the raw file tests something the app never does.
- **Filenames contain U+202F** (narrow no-break space) and do not survive being retyped.
  Fixture names are resolved through `resolve()` — exact, then a glob with spaces widened
  to `?`, then a whitespace-normalised scan. Never compare a fixture string to a filename
  directly.
- **`meta.db` is WAL and the app writes to it while tests run.** `db_connect()` tries
  read-only, then immutable, then a copy. It never opens the live database read-write.

## Updating the fixtures

Expected values live in [`scripts/cases.json`](scripts/cases.json), separate from the
harness, because the library changes. Each expectation carries a `baseline` string
recording what it measured historically and when — keep appending to those rather than
overwriting, so drift stays visible. The colourway pair count is derived from a glob, not
hard-coded, for the same reason.

When a case legitimately changes (a new library, a deliberate tuning change), update the
`baseline` with the date and the cause. When one fails unexpectedly, the baseline is what
tells you how far it moved and what moved it.
