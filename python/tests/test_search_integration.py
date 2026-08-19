"""
End-to-end test of the reported bug, against a real FAISS index and SQLite DB.

The real CLIP weights are encrypted and fetched at login, so they cannot run
here. They are replaced by a stand-in embedder that shares the property this
bug is about: it is *orientation-sensitive*, so a mirrored or rotated copy of
an image lands somewhere else in embedding space. Everything else on the path —
folder scan, hashing, preprocessing, FAISS add/search, the orientation
expansion and the merge — is the production code.
"""

import os
import sys
import types

import numpy as np
import pytest
from PIL import Image

PYTHON_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, PYTHON_ROOT)

pytest.importorskip("faiss", reason="faiss-cpu is required for the integration test")


# ── stub the optional heavy deps the search path imports but does not use ──
def _install_stubs():
    if "onnxruntime" not in sys.modules:
        sys.modules["onnxruntime"] = types.ModuleType("onnxruntime")

    if "psd_tools" not in sys.modules:
        psd_tools = types.ModuleType("psd_tools")
        psd_tools.PSDImage = object
        sys.modules["psd_tools"] = psd_tools

    try:
        import cryptography.fernet  # noqa: F401
    except BaseException:
        # Not just ImportError: a half-installed cryptography raises a pyo3
        # PanicException here, and this test needs no real crypto either way.
        crypto = types.ModuleType("cryptography")
        fernet = types.ModuleType("cryptography.fernet")
        fernet.Fernet = object
        crypto.fernet = fernet
        sys.modules["cryptography"] = crypto
        sys.modules["cryptography.fernet"] = fernet


_install_stubs()

from app.config import CLIP_MEAN, CLIP_STD, EMB_DIM  # noqa: E402
from app.core import database as db_module  # noqa: E402
from app.core import indexer as indexer_module  # noqa: E402
from app.core.embedder import Embedder  # noqa: E402
from app.services import search_service  # noqa: E402
from app.utils.orientation import ORIENTATIONS  # noqa: E402

POOL = 14  # 224 / 14 = 16, and 16 * 16 * 3 == 768 == EMB_DIM


class StandInEmbedder:
    """
    Deterministic, orientation-sensitive stand-in for the CLIP session.

    Average-pools the preprocessed tensor down to a 16x16 RGB grid and uses that
    as the embedding. Like CLIP it preserves spatial layout, so a flipped image
    embeds differently; unlike CLIP it needs no weights.
    """

    def __init__(self):
        self.mean = np.array(CLIP_MEAN, dtype=np.float32)
        self.std  = np.array(CLIP_STD,  dtype=np.float32)

    def embed_batch(self, batch: np.ndarray) -> np.ndarray:
        n, c, h, w = batch.shape
        pooled = batch.reshape(n, c, h // POOL, POOL, w // POOL, POOL).mean(axis=(3, 5))
        flat   = pooled.reshape(n, -1).astype(np.float32)
        assert flat.shape[1] == EMB_DIM
        norms = np.linalg.norm(flat, axis=1, keepdims=True)
        return (flat / norms).astype(np.float32)


def _structured_image(seed: int, width: int = 600, height: int = 1200) -> Image.Image:
    """
    A tile-like image with no rotational or mirror symmetry, so that every
    orientation of it is genuinely distinguishable.
    """
    rng = np.random.default_rng(seed)
    yy, xx = np.mgrid[0:height, 0:width].astype(np.float32)
    yy /= height
    xx /= width

    base = np.stack([
        200 + 40 * np.sin(6 * xx + seed) * yy,
        190 + 45 * (xx ** 2) + 20 * yy,
        180 + 50 * yy * (1 - xx),
    ], axis=-1)

    # A hard asymmetric marker: bright block in one corner, dark in the opposite.
    base[: height // 5, : width // 3] = 250
    base[-height // 4:, -width // 2:] = 40
    base += rng.normal(0, 3, size=base.shape)

    return Image.fromarray(np.clip(base, 0, 255).astype(np.uint8), mode="RGB")


@pytest.fixture
def library(tmp_path, monkeypatch):
    """A folder of indexed tiles plus a mirrored+rotated copy of 59.jpg."""
    faiss_dir = tmp_path / "faiss"
    faiss_dir.mkdir()
    folder = tmp_path / "VARMORA"
    folder.mkdir()
    queries = tmp_path / "uploads"
    queries.mkdir()

    monkeypatch.setattr(db_module, "FAISS_DIR", str(faiss_dir))
    monkeypatch.setattr(db_module, "DB_PATH", str(faiss_dir / "meta.db"))
    monkeypatch.setattr(indexer_module, "FAISS_DIR", str(faiss_dir))
    monkeypatch.setattr(indexer_module, "INDEX_PATH", str(faiss_dir / "index.faiss"))

    monkeypatch.setattr(Embedder, "_instance", StandInEmbedder())

    for n in range(55, 65):
        _structured_image(seed=n).save(folder / f"{n}.jpg", quality=95)

    # The upload: 59.jpg mirrored and turned 180 degrees, exactly the case the
    # user hit. Saved outside the searched folder so it is not itself indexed.
    target = Image.open(folder / "59.jpg")
    flipped = target.transpose(Image.FLIP_LEFT_RIGHT).transpose(Image.ROTATE_180)
    query_path = queries / "user_upload.jpg"
    flipped.save(query_path, quality=95)

    return types.SimpleNamespace(
        folder     = str(folder),
        query      = str(query_path),
        target     = os.path.normpath(str(folder / "59.jpg")),
        upright    = str(queries / "upright.jpg"),
    )


def _paths(result: dict):
    return [r["path"] for r in result["data"]["results"]]


# ── the bug ─────────────────────────────────────────────────────────────

def test_mirrored_rotated_upload_is_missed_without_orientation_expansion(library, monkeypatch):
    """
    Reproduces the reported failure. Searching upright-only, the tile the upload
    was actually made from does not read as a match at all: it scores like an
    unrelated image and several unrelated tiles outrank it.
    """
    monkeypatch.setattr(search_service, "SEARCH_ORIENTATIONS", 1)

    result = search_service.search(library.query, library.folder, top_k=10)
    results = result["data"]["results"]

    assert result["success"] is True
    assert results, "expected the folder to be indexed and searchable"

    hit = next(r for r in results if r["path"] == library.target)
    assert hit["rank"] > 1, "premise broken: the upright search already ranks it first"
    assert hit["similarity"] < 0.9, "an exact match would score far higher than this"


def test_mirrored_rotated_upload_is_found_with_orientation_expansion(library, monkeypatch):
    """The fix: the same upload now returns 59.jpg as the top match."""
    monkeypatch.setattr(search_service, "SEARCH_ORIENTATIONS", 8)

    result = search_service.search(library.query, library.folder, top_k=10)

    assert result["success"] is True
    top = result["data"]["results"][0]
    assert top["path"] == library.target
    assert top["similarity"] > 0.99, "an exact re-orientation should be a near-perfect match"
    assert top["orientation"] == "mirror_rot180"
    assert top["orientation_label"] == "Mirrored + rotated 180°"


# ── it must not regress the ordinary case ───────────────────────────────

def test_upright_upload_still_matches_itself_first(library, monkeypatch):
    """Expanding the query must not push an ordinary upright match off the top."""
    monkeypatch.setattr(search_service, "SEARCH_ORIENTATIONS", 8)
    Image.open(library.target).save(library.upright, quality=95)

    result = search_service.search(library.upright, library.folder, top_k=10)

    top = result["data"]["results"][0]
    assert top["path"] == library.target
    assert top["orientation"] == "original"


def test_each_file_appears_at_most_once(library, monkeypatch):
    monkeypatch.setattr(search_service, "SEARCH_ORIENTATIONS", 8)

    result = search_service.search(library.query, library.folder, top_k=50)

    paths = _paths(result)
    assert len(paths) == len(set(paths))


def test_results_stay_ranked_and_capped_at_top_k(library, monkeypatch):
    monkeypatch.setattr(search_service, "SEARCH_ORIENTATIONS", 8)

    result = search_service.search(library.query, library.folder, top_k=3)

    results = result["data"]["results"]
    assert len(results) == 3
    assert [r["rank"] for r in results] == [1, 2, 3]
    sims = [r["similarity"] for r in results]
    assert sims == sorted(sims, reverse=True)


def test_response_reports_which_orientations_were_searched(library, monkeypatch):
    monkeypatch.setattr(search_service, "SEARCH_ORIENTATIONS", 8)

    result = search_service.search(library.query, library.folder, top_k=5)

    assert result["data"]["orientations_searched"] == list(ORIENTATIONS)


# How the upload was transformed -> the orientation that undoes it. Rotations
# are undone by the opposite rotation; every mirrored variant is its own inverse.
_UPLOAD_TO_MATCHING_ORIENTATION = {
    "rot90":         "rot270",
    "rot180":        "rot180",
    "rot270":        "rot90",
    "mirror":        "mirror",
    "mirror_rot90":  "mirror_rot90",
    "mirror_rot180": "mirror_rot180",
    "mirror_rot270": "mirror_rot270",
}

_UPLOAD_CHAINS = {
    "rot90":         (Image.ROTATE_90,),
    "rot180":        (Image.ROTATE_180,),
    "rot270":        (Image.ROTATE_270,),
    "mirror":        (Image.FLIP_LEFT_RIGHT,),
    "mirror_rot90":  (Image.FLIP_LEFT_RIGHT, Image.ROTATE_90),
    "mirror_rot180": (Image.FLIP_LEFT_RIGHT, Image.ROTATE_180),
    "mirror_rot270": (Image.FLIP_LEFT_RIGHT, Image.ROTATE_270),
}


@pytest.mark.parametrize("upload_transform", sorted(_UPLOAD_CHAINS))
def test_any_reorientation_of_a_tile_finds_it(library, monkeypatch, upload_transform):
    """
    However the user's copy was turned or flipped, it must resolve back to the
    tile it came from — and be reported under the orientation that undoes it.
    """
    monkeypatch.setattr(search_service, "SEARCH_ORIENTATIONS", 8)

    img = Image.open(library.target)
    for op in _UPLOAD_CHAINS[upload_transform]:
        img = img.transpose(op)
    probe = os.path.join(os.path.dirname(library.query), f"probe_{upload_transform}.jpg")
    img.save(probe, quality=95)

    result = search_service.search(probe, library.folder, top_k=5)
    top = result["data"]["results"][0]

    assert top["path"] == library.target, f"{upload_transform} upload lost its source tile"
    assert top["similarity"] > 0.99
    assert top["orientation"] == _UPLOAD_TO_MATCHING_ORIENTATION[upload_transform]


# ── input validation ────────────────────────────────────────────────────

def test_missing_query_image_reports_cleanly(library):
    result = search_service.search(
        os.path.join(os.path.dirname(library.query), "nope.jpg"), library.folder, top_k=5
    )
    assert result["success"] is False
    assert result["message"] == "Query image not found."


def test_missing_folder_reports_cleanly(library):
    result = search_service.search(library.query, library.folder + "_gone", top_k=5)
    assert result["success"] is False
    assert result["message"] == "Folder not found."
