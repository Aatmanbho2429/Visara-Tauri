# Parity: verify_one's result dicts must match the golden output captured from the pre-refactor implementation.
import json
import os
import sys
import tempfile
import unittest

import cv2
import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
sys.path.insert(0, os.path.dirname(__file__))

import pipeline  # noqa: E402
from synth import texture  # noqa: E402

GOLDEN = os.path.join(os.path.dirname(__file__), "golden_verify.json")


# Rotated, scaled centre crop of `img` (a rotated crop of the query region, as a real "found inside" case).
def rotated_crop(img, angle=17.0, frac=0.6):
    h, w = img.shape[:2]
    ch, cw = int(h * frac), int(w * frac)
    y0, x0 = (h - ch) // 2, (w - cw) // 2
    crop = img[y0:y0 + ch, x0:x0 + cw]
    m = cv2.getRotationMatrix2D((cw / 2, ch / 2), angle, 1.0)
    return cv2.warpAffine(crop, m, (cw, ch), borderMode=cv2.BORDER_REFLECT)


# (name, query image, candidate image, mirror flag) — every branch of verify_one that a synthetic pair can reach.
def cases():
    a, b = texture(11), texture(12)
    flat = np.full_like(a, 127)
    return [
        ("identical", a, a, False),
        ("rotated_crop_candidate_is_big", rotated_crop(a, 17.0), a, False),
        ("query_is_crop_of_candidate", a[60:300, 80:400], a, False),
        ("resized_half", cv2.resize(a, (240, 180), interpolation=cv2.INTER_AREA), a, False),
        ("rot90", np.ascontiguousarray(np.rot90(a)), a, False),
        ("mirrored_query_flag", np.ascontiguousarray(a[:, ::-1]), a, True),
        ("mirrored_query_no_flag", np.ascontiguousarray(a[:, ::-1]), a, False),
        ("unrelated", b, a, False),
        ("flat_candidate", a, flat, False),
        ("flat_query", flat, a, False),
    ]


# Runs every case through the live `verify_one` and returns {name: result dict}.
def run_all():
    out = {}
    with tempfile.TemporaryDirectory() as d:
        for name, q, c, mirror in cases():
            qp, cp = os.path.join(d, f"{name}_q.png"), os.path.join(d, f"{name}_c.png")
            cv2.imwrite(qp, cv2.cvtColor(q, cv2.COLOR_RGB2BGR))
            cv2.imwrite(cp, cv2.cvtColor(c, cv2.COLOR_RGB2BGR))
            state = pipeline.prepare_query(qp, mirror=mirror)
            out[name] = json.loads(json.dumps(pipeline.verify_one(state, cp)))
    return out


class VerifyParityTest(unittest.TestCase):
    # The refactored verify_one reproduces the recorded results exactly.
    def test_matches_golden(self):
        with open(GOLDEN) as f:
            golden = json.load(f)
        got = run_all()
        self.assertEqual(sorted(got), sorted(golden))
        for name in golden:
            self.assertEqual(got[name], golden[name], name)

    # The golden set exercises both outcomes, so a refactor cannot pass by always returning one of them.
    def test_golden_covers_match_and_reject(self):
        with open(GOLDEN) as f:
            golden = json.load(f)
        self.assertTrue(any(r.get("matched") for r in golden.values()))
        self.assertTrue(any(not r.get("matched") for r in golden.values()))


if __name__ == "__main__":
    if "--record" in sys.argv:
        with open(GOLDEN, "w") as f:
            json.dump(run_all(), f, indent=1, sort_keys=True)
        print("recorded", GOLDEN)
    else:
        unittest.main()
