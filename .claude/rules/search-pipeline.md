---
paths:
  - "UI/src-tauri/src/services/search/**/*.rs"
  - "UI/src-tauri/src/core/vector_store.rs"
  - "UI/src-tauri/src/core/search_gate.rs"
---

# Search pipeline (`services::search`)

1. Describe the query image via the sidecar (`/describe`).
2. Select the **near family**: every indexed file whose DINO embedding cosine similarity is
   `>= config::NEAR_FAMILY_MIN_SIM`. A floor, not a top-N rank cut — the old fixed 1500-item
   shortlist could silently exclude a real match in a large library. Computed in-process against
   `VectorStore`.
3. Geometrically verify the *entire* near family via the sidecar's `/verify` (SIFT/RANSAC). This
   backs the `verified`/`partial`/`mirrored` flags and the match-point count in `SearchResult`.
   Nothing is truncated here either.
4. Secondary Gabor-rose + Gram-matrix score (`ROSE_WEIGHT`/`GRAM_WEIGHT`) is still computed and
   returned, but no longer selects candidates — it only breaks ties at equal embedding cosine.

## Watch the floor

`NEAR_FAMILY_MIN_SIM` (0.70) is the only bound on how much work a search does, and it is not
calibrated against the current model. The number to watch is `near_family_n` in the
`[timing] search TOTAL` log line — if it's a large fraction of the library on an ordinary query,
the floor is too low and every downstream stage pays for it.

Don't hard-code a replacement threshold from local results. The only library available locally is
~400 images; calibration needs real data.

## Reporting timings

`[timing] search TOTAL` carries `near_family_n`, `verified_n`, `describe_ms`, `near_family_ms`,
`verify_ms`, `total_ms`. It's the only way to tell whether a slow search is the sidecar, the
near-family scan, or SIFT. Quote a real line from `Pictoria.log` — never an estimate, and never a
number from a debug build presented as a release cost.
