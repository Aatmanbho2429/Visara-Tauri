"""
Tests for merging per-orientation FAISS result rows into one ranked list.

`app.services.search_service` pulls in faiss / onnxruntime / psd_tools at import
time, none of which are needed by the merge itself, so the function is loaded
from source in isolation.
"""

import os
import sys
import types

import numpy as np
import pytest

PYTHON_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, PYTHON_ROOT)

from app.utils.orientation import ORIENTATION_LABELS, ORIENTATIONS  # noqa: E402


def _load_merge_fn():
    """Exec just the merge helper against the real orientation labels."""
    source = open(
        os.path.join(PYTHON_ROOT, "app", "services", "search_service.py"),
        encoding="utf-8",
    ).read()

    start = source.index("def _merge_orientation_hits")
    end   = source.index("def search(")
    module = types.ModuleType("_merge_only")
    module.__dict__["ORIENTATION_LABELS"] = ORIENTATION_LABELS
    exec(compile(source[start:end], "search_service.py", "exec"), module.__dict__)
    return module._merge_orientation_hits


merge = _load_merge_fn()

ID_MAP = {
    10: r"D:\ImageDb\VARMORA\59.jpg",
    11: r"D:\ImageDb\VARMORA\60.jpg",
    12: r"D:\ImageDb\VARMORA\61.jpg",
}


def _rows(per_orientation):
    """Build (scores, ids, names) from {orientation: [(faiss_id, score), ...]}."""
    names  = tuple(per_orientation)
    width  = max(len(v) for v in per_orientation.values())
    scores = np.full((len(names), width), -1.0, dtype=np.float32)
    ids    = np.full((len(names), width), -1,   dtype=np.int64)

    for row, name in enumerate(names):
        for col, (faiss_id, score) in enumerate(per_orientation[name]):
            ids[row][col]    = faiss_id
            scores[row][col] = score
    return scores, ids, names


# ── the reported bug ────────────────────────────────────────────────────

def test_mirrored_rotated_query_finds_the_file_the_upright_query_misses():
    """
    The user's case: the query is 59.jpg mirrored and turned 180 degrees.
    Upright, it scores poorly and 59.jpg never surfaces; under the matching
    orientation it is a near-exact hit and must come back first.
    """
    scores, ids, names = _rows({
        "original":      [(11, 0.61), (12, 0.58)],
        "mirror_rot180": [(10, 0.97), (11, 0.60)],
    })

    results = merge(scores, ids, names, ID_MAP)

    assert results[0]["path"] == ID_MAP[10]
    assert results[0]["rank"] == 1
    assert results[0]["similarity"] == pytest.approx(0.97)
    assert results[0]["orientation"] == "mirror_rot180"
    assert results[0]["orientation_label"] == "Mirrored + rotated 180°"


def test_upright_only_search_still_misses_it():
    """Guards the premise above: with no expansion, 59.jpg is simply absent."""
    scores, ids, names = _rows({"original": [(11, 0.61), (12, 0.58)]})

    results = merge(scores, ids, names, ID_MAP)

    assert ID_MAP[10] not in [r["path"] for r in results]


# ── merge semantics ─────────────────────────────────────────────────────

def test_file_matched_by_several_orientations_appears_once_at_its_best_score():
    scores, ids, names = _rows({
        "original":     [(10, 0.55)],
        "rot90":        [(10, 0.71)],
        "mirror":       [(10, 0.93)],
        "mirror_rot90": [(10, 0.66)],
    })

    results = merge(scores, ids, names, ID_MAP)

    assert len(results) == 1
    assert results[0]["similarity"] == pytest.approx(0.93)
    assert results[0]["orientation"] == "mirror"


def test_results_are_ranked_by_descending_similarity():
    scores, ids, names = _rows({
        "original": [(12, 0.40), (11, 0.80)],
        "rot180":   [(10, 0.60)],
    })

    results = merge(scores, ids, names, ID_MAP)

    assert [r["path"] for r in results] == [ID_MAP[11], ID_MAP[10], ID_MAP[12]]
    assert [r["rank"] for r in results] == [1, 2, 3]
    sims = [r["similarity"] for r in results]
    assert sims == sorted(sims, reverse=True)


def test_upright_match_keeps_its_orientation_label():
    scores, ids, names = _rows({"original": [(10, 0.99)], "mirror": [(10, 0.31)]})

    results = merge(scores, ids, names, ID_MAP)

    assert results[0]["orientation"] == "original"
    assert results[0]["orientation_label"] == "Upright"


# ── filtering and padding ───────────────────────────────────────────────

def test_faiss_padding_ids_are_skipped():
    scores, ids, names = _rows({"original": [(10, 0.90), (-1, -1.0), (-1, -1.0)]})

    results = merge(scores, ids, names, ID_MAP)

    assert len(results) == 1
    assert results[0]["path"] == ID_MAP[10]


def test_hits_outside_the_selected_folder_are_dropped():
    """The index spans every synced folder; only the searched one may return."""
    scores, ids, names = _rows({
        "original": [(999, 0.99), (10, 0.70)],
        "mirror":   [(998, 0.98)],
    })

    results = merge(scores, ids, names, ID_MAP)

    assert [r["path"] for r in results] == [ID_MAP[10]]


def test_no_hits_yields_empty_list():
    scores, ids, names = _rows({"original": [(-1, -1.0)]})

    assert merge(scores, ids, names, ID_MAP) == []


def test_every_orientation_name_resolves_to_a_label():
    scores, ids, names = _rows({name: [(10, i / 10)] for i, name in enumerate(ORIENTATIONS)})

    results = merge(scores, ids, names, ID_MAP)

    assert len(results) == 1
    assert results[0]["orientation_label"] != results[0]["orientation"]


def test_scores_and_ids_are_plain_python_types():
    """The response is JSON-serialised, so numpy scalars must not leak out."""
    import json

    scores, ids, names = _rows({"original": [(10, 0.9)], "rot90": [(11, 0.5)]})

    results = merge(scores, ids, names, ID_MAP)

    for r in results:
        assert isinstance(r["similarity"], float)
        assert isinstance(r["rank"], int)
    json.dumps(results)  # must not raise
