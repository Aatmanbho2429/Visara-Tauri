# Fingerprint checks: rotated/mirrored copies land within Hamming 4 at the matching variant, flat images get none.
import os
import sys
import unittest

import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
sys.path.insert(0, os.path.dirname(__file__))

import pipeline  # noqa: E402
from synth import texture  # noqa: E402


def ham(a, b):
    return bin(int(a, 16) ^ int(b, 16)).count("1")


class FingerprintTest(unittest.TestCase):
    def setUp(self):
        self.img = texture(1)
        self.fp = pipeline.fingerprint(self.img)

    # Eight 16-char lowercase hex strings, variant 0 first.
    def test_shape(self):
        self.assertEqual(len(self.fp), 8)
        for h in self.fp:
            self.assertEqual(len(h), 16)
            int(h, 16)

    # np.rot90 copies hash to variant k of the original (the spec's "within 4 at the matching variant index").
    def test_rotations_match_their_variant(self):
        for k in range(4):
            rotated = pipeline.fingerprint(np.ascontiguousarray(np.rot90(self.img, k)))
            self.assertLessEqual(ham(self.fp[k], rotated[0]), 4, f"rot90 x{k}")

    # A horizontal mirror hashes to variant 4 (the fliplr slot).
    def test_mirror_matches_variant_4(self):
        mirrored = pipeline.fingerprint(np.ascontiguousarray(self.img[:, ::-1]))
        self.assertLessEqual(ham(self.fp[4], mirrored[0]), 4)

    # An unrelated texture is far from every variant.
    def test_unrelated_is_far(self):
        other = pipeline.fingerprint(texture(2))
        self.assertGreater(min(ham(a, other[0]) for a in self.fp), 10)

    # A flat image has no fingerprint, so plain images never collide.
    def test_flat_image_returns_empty(self):
        self.assertEqual(pipeline.fingerprint(np.full((120, 160, 3), 128, np.uint8)), [])


if __name__ == "__main__":
    unittest.main()
