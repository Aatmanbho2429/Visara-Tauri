"""
Orientation-invariant query expansion.

CLIP embeddings are *not* rotation- or mirror-invariant: a tile photographed
(or exported) upside-down, or flipped, lands far from its own upright copy in
embedding space. A user searching with a mirrored + rotated crop of an indexed
file therefore gets no exact match, even though the file is right there in the
folder.

The fix is test-time augmentation on the *query* only: we expand one query
image into the 8 elements of the dihedral group D4 — 4 rotations, each with and
without a horizontal mirror — embed all 8, and search the index with every one
of them. The index keeps exactly one vector per file, so nothing has to be
re-indexed and the folder-load cost is unchanged; only the query gets 8x more
expensive, which is 8 forward passes on a single image.

Why transform the preprocessed tensor instead of the source image
----------------------------------------------------------------
`preprocess_single` does: resize to a fixed 224x224 square -> scale to [0,1] ->
per-channel (x - mean) / std -> transpose to CHW.

Every one of those steps commutes with a D4 transform:

  * the resize target is *square*, so swapping the H and W axes before or after
    it gives the same 224x224 grid (this is what makes rot90 safe even for
    non-square source images, where the resize is aspect-distorting);
  * scaling and the per-channel normalisation are pointwise, and D4 transforms
    only permute pixel positions, never channels.

So rotating the (3,224,224) tensor is equivalent to rotating the source image
and re-running the whole pipeline, at the cost of one array copy instead of a
decode + resize.

That equivalence is bit-exact for the four axis-preserving transforms
(original, rot180, mirror, mirror_rot180) at any source size, and for all
eight when the source is already square. For rot90/rot270 on a *non-square*
source the two paths disagree by at most one 8-bit quantisation step
(1/255, i.e. ~0.015 after normalisation) because Pillow rounds an upscaled and
a downscaled axis differently — a pixel-space cosine of 0.99997, far below
anything CLIP resolves. `tests/test_orientation.py` pins both bounds.
"""

import numpy as np

# Ordered so that a prefix of the list is still a sensible set:
# [:1] = upright only, [:4] = rotations only, [:8] = rotations + mirrors.
ORIENTATIONS = (
    "original",
    "rot90",
    "rot180",
    "rot270",
    "mirror",
    "mirror_rot90",
    "mirror_rot180",
    "mirror_rot270",
)

# Human-readable labels for the UI / activity log.
ORIENTATION_LABELS = {
    "original":      "Upright",
    "rot90":         "Rotated 90°",
    "rot180":        "Rotated 180°",
    "rot270":        "Rotated 270°",
    "mirror":        "Mirrored",
    "mirror_rot90":  "Mirrored + rotated 90°",
    "mirror_rot180": "Mirrored + rotated 180°",
    "mirror_rot270": "Mirrored + rotated 270°",
}

# (mirror?, quarter-turns) for each name above.
_TRANSFORMS = {
    "original":      (False, 0),
    "rot90":         (False, 1),
    "rot180":        (False, 2),
    "rot270":        (False, 3),
    "mirror":        (True,  0),
    "mirror_rot90":  (True,  1),
    "mirror_rot180": (True,  2),
    "mirror_rot270": (True,  3),
}


def resolve_orientations(count: int) -> tuple:
    """
    Map the SEARCH_ORIENTATIONS config value onto actual orientation names.

    1 -> upright only (original behaviour, no expansion)
    4 -> the four rotations
    8 -> the full dihedral group (rotations + mirrors)

    Anything else is clamped into [1, 8].
    """
    try:
        count = int(count)
    except (TypeError, ValueError):
        count = len(ORIENTATIONS)
    count = max(1, min(count, len(ORIENTATIONS)))
    return ORIENTATIONS[:count]


def transform(arr: np.ndarray, orientation: str) -> np.ndarray:
    """
    Apply one D4 transform to a CHW image tensor.

    arr: (C, H, W) float array — the output of `preprocess_single`.
    Returns a new C-contiguous array of the same dtype; for a square input
    (which 224x224 always is) the shape is unchanged.
    """
    if orientation not in _TRANSFORMS:
        raise ValueError(f"Unknown orientation: {orientation}")

    if arr.ndim != 3:
        raise ValueError(f"Expected a (C,H,W) tensor, got shape {arr.shape}")

    mirror, turns = _TRANSFORMS[orientation]

    out = arr[:, :, ::-1] if mirror else arr
    if turns:
        # axes=(1,2) rotates within the spatial plane, leaving channels alone.
        out = np.rot90(out, turns, axes=(1, 2))

    return np.ascontiguousarray(out)


def build_orientation_batch(arr: np.ndarray, orientations=None) -> tuple:
    """
    Expand one preprocessed image into a batch of its orientations.

    Returns (batch, names) where batch is (N, C, H, W) float32 ready for
    `Embedder.embed_batch`, and names[i] identifies the transform in row i.
    """
    names = tuple(orientations) if orientations else ORIENTATIONS
    batch = np.stack([transform(arr, name) for name in names])
    return batch.astype(np.float32), names
