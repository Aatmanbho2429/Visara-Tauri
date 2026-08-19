"""
Tests for the dihedral query expansion used by orientation-invariant search.

These deliberately avoid importing `app.utils.image_loader` / `app.core.embedder`
so they run without onnxruntime, faiss or psd_tools installed — the property
under test is pure array maths.
"""

import os
import sys

import numpy as np
import pytest
from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from app.config import CLIP_MEAN, CLIP_STD  # noqa: E402
from app.utils.orientation import (  # noqa: E402
    ORIENTATIONS,
    ORIENTATION_LABELS,
    build_orientation_batch,
    resolve_orientations,
    transform,
)

MEAN = np.array(CLIP_MEAN, dtype=np.float32)
STD  = np.array(CLIP_STD,  dtype=np.float32)

# PIL equivalent of each named transform, as a chain of Image.transpose ops.
_PIL_CHAIN = {
    "original":      (),
    "rot90":         (Image.ROTATE_90,),
    "rot180":        (Image.ROTATE_180,),
    "rot270":        (Image.ROTATE_270,),
    "mirror":        (Image.FLIP_LEFT_RIGHT,),
    "mirror_rot90":  (Image.FLIP_LEFT_RIGHT, Image.ROTATE_90),
    "mirror_rot180": (Image.FLIP_LEFT_RIGHT, Image.ROTATE_180),
    "mirror_rot270": (Image.FLIP_LEFT_RIGHT, Image.ROTATE_270),
}


def _preprocess(img: Image.Image) -> np.ndarray:
    """Mirror of image_loader.preprocess_single, without the model dependency."""
    img = img.resize((224, 224), Image.BILINEAR)
    arr = np.array(img, dtype=np.float32) / 255.0
    arr = (arr - MEAN) / STD
    return np.transpose(arr, (2, 0, 1))


def _pil_transform(img: Image.Image, orientation: str) -> Image.Image:
    for op in _PIL_CHAIN[orientation]:
        img = img.transpose(op)
    return img


def _sample_image(width: int, height: int) -> Image.Image:
    """An image with no rotational or mirror symmetry, so every transform differs."""
    rng = np.random.default_rng(20240819)
    data = rng.integers(0, 256, size=(height, width, 3), dtype=np.uint8)
    # Add a hard corner marker so symmetry breaking is unambiguous.
    data[: height // 4, : width // 4] = 255
    data[-height // 4:, -width // 4:] = 0
    return Image.fromarray(data, mode="RGB")


# ── the group itself ────────────────────────────────────────────────────

def test_eight_orientations_are_the_full_dihedral_group():
    assert len(ORIENTATIONS) == 8
    assert len(set(ORIENTATIONS)) == 8
    assert set(ORIENTATION_LABELS) == set(ORIENTATIONS)


def test_every_orientation_produces_a_distinct_tensor():
    arr = _preprocess(_sample_image(320, 200))
    seen = {name: transform(arr, name) for name in ORIENTATIONS}

    fingerprints = {name: t.tobytes() for name, t in seen.items()}
    assert len(set(fingerprints.values())) == 8, "orientations collapsed onto each other"


def test_transforms_are_involutive_or_cyclic():
    arr = _preprocess(_sample_image(256, 256))

    # A mirror applied twice is the identity.
    assert np.array_equal(transform(transform(arr, "mirror"), "mirror"), arr)

    # Four quarter turns return to the start.
    spun = arr
    for _ in range(4):
        spun = transform(spun, "rot90")
    assert np.array_equal(spun, arr)

    # rot180 is rot90 twice.
    assert np.array_equal(transform(arr, "rot180"), transform(transform(arr, "rot90"), "rot90"))


def test_transform_preserves_shape_and_contiguity():
    arr = _preprocess(_sample_image(400, 150))
    for name in ORIENTATIONS:
        out = transform(arr, name)
        assert out.shape == arr.shape
        assert out.flags["C_CONTIGUOUS"], f"{name} produced a non-contiguous view"


def test_unknown_orientation_rejected():
    arr = _preprocess(_sample_image(64, 64))
    with pytest.raises(ValueError):
        transform(arr, "rot45")


def test_non_chw_input_rejected():
    with pytest.raises(ValueError):
        transform(np.zeros((224, 224), dtype=np.float32), "rot90")


# ── the load-bearing claim: tensor transform == source-image transform ──

SIZES = [(224, 224), (320, 200), (200, 320), (601, 149), (1200, 600)]

# Transforms that keep each source axis on the same output axis. These commute
# with the resize exactly, whatever the source aspect ratio.
AXIS_PRESERVING = ("original", "rot180", "mirror", "mirror_rot180")
# Transforms that swap the H and W axes. Exact only when the source is square.
AXIS_SWAPPING   = ("rot90", "rot270", "mirror_rot90", "mirror_rot270")

# One 8-bit quantisation step, expanded by the tightest CLIP channel std.
ONE_LEVEL = (1 / 255) / min(CLIP_STD)


def test_axis_classes_cover_every_orientation():
    assert set(AXIS_PRESERVING) | set(AXIS_SWAPPING) == set(ORIENTATIONS)


@pytest.mark.parametrize("size", SIZES)
@pytest.mark.parametrize("orientation", AXIS_PRESERVING)
def test_axis_preserving_transform_is_bit_exact(size, orientation):
    """
    Mirroring or half-turning the preprocessed tensor must be bit-identical to
    doing it to the source image and re-running preprocessing. This is what
    lets us expand the query from a single decode instead of eight.
    """
    img = _sample_image(*size)

    from_tensor = transform(_preprocess(img), orientation)
    from_source = _preprocess(_pil_transform(img, orientation))

    assert from_tensor.shape == from_source.shape
    np.testing.assert_array_equal(from_tensor, from_source)


@pytest.mark.parametrize("orientation", AXIS_SWAPPING)
def test_axis_swapping_transform_is_bit_exact_on_square_sources(orientation):
    img = _sample_image(224, 224)

    from_tensor = transform(_preprocess(img), orientation)
    from_source = _preprocess(_pil_transform(img, orientation))

    np.testing.assert_array_equal(from_tensor, from_source)


@pytest.mark.parametrize("size", SIZES)
@pytest.mark.parametrize("orientation", AXIS_SWAPPING)
def test_axis_swapping_transform_stays_within_one_quantisation_step(size, orientation):
    """
    A quarter turn swaps which source axis is upscaled and which is downscaled,
    and Pillow rounds those two directions differently. The gap is bounded by a
    single 8-bit level, which is far below anything CLIP resolves — so the cheap
    tensor-space path stays a faithful stand-in for re-decoding the image.
    """
    img = _sample_image(*size)

    from_tensor = transform(_preprocess(img), orientation)
    from_source = _preprocess(_pil_transform(img, orientation))

    assert from_tensor.shape == from_source.shape
    np.testing.assert_allclose(from_tensor, from_source, atol=ONE_LEVEL * 1.01)

    flat_a, flat_b = from_tensor.ravel(), from_source.ravel()
    cosine = float(flat_a @ flat_b / (np.linalg.norm(flat_a) * np.linalg.norm(flat_b)))
    assert cosine > 0.9999


# ── batch construction ──────────────────────────────────────────────────

def test_build_orientation_batch_shape_and_order():
    arr = _preprocess(_sample_image(320, 200))
    batch, names = build_orientation_batch(arr)

    assert batch.shape == (8, 3, 224, 224)
    assert batch.dtype == np.float32
    assert names == ORIENTATIONS
    # Row 0 must be the untouched query, so an upright match still scores first.
    np.testing.assert_allclose(batch[0], arr, atol=0)


def test_build_orientation_batch_honours_subset():
    arr = _preprocess(_sample_image(128, 128))
    batch, names = build_orientation_batch(arr, ("original", "mirror"))

    assert batch.shape == (2, 3, 224, 224)
    assert names == ("original", "mirror")


# ── config resolution ───────────────────────────────────────────────────

@pytest.mark.parametrize("value,expected", [
    (1, 1), (4, 4), (8, 8),
    (0, 1), (-3, 1), (99, 8),
    ("8", 8), (None, 8), ("nonsense", 8),
])
def test_resolve_orientations_clamps(value, expected):
    assert len(resolve_orientations(value)) == expected


def test_resolve_orientations_prefixes_are_meaningful():
    assert resolve_orientations(1) == ("original",)
    assert resolve_orientations(4) == ("original", "rot90", "rot180", "rot270")
    assert resolve_orientations(8) == ORIENTATIONS
