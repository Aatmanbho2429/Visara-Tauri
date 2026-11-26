# Stored-feature engine checks: extraction shape, wire round-trip, feature verify vs on-the-fly verify, server job kinds.
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
from test_verify_parity import cases  # noqa: E402


class ExtractTest(unittest.TestCase):
    # At most MATCH_FEATURES_PER_IMAGE keypoints, ordered by response, uint8 descriptors, decoded size reported as (w, h).
    def test_shapes_and_limits(self):
        img = texture(3)
        pts, sizes, angles, desc, (w, h) = pipeline.extract_features(img)
        n = len(pts)
        self.assertTrue(0 < n <= pipeline.MATCH_FEATURES_PER_IMAGE)
        self.assertEqual((pts.shape, sizes.shape, angles.shape, desc.shape), ((n, 2), (n,), (n,), (n, 128)))
        self.assertEqual((pts.dtype, sizes.dtype, angles.dtype, desc.dtype), (np.float32, np.float32, np.float32, np.uint8))
        self.assertEqual((w, h), (img.shape[1], img.shape[0]))
        self.assertTrue(pts[:, 0].max() < w and pts[:, 1].max() < h)

    # A featureless image yields zero keypoints rather than an error.
    def test_flat_image_has_no_features(self):
        pts, _, _, desc, _ = pipeline.extract_features(np.full((100, 120, 3), 90, np.uint8))
        self.assertEqual((len(pts), desc.shape), (0, (0, 128)))

    # Base64 little-endian payloads decode back to the same arrays.
    def test_b64_round_trip(self):
        a = np.arange(12, dtype=np.float32).reshape(6, 2) * 1.5
        np.testing.assert_array_equal(pipeline.unb64_array(pipeline.b64_array(a, "<f4"), "<f4").reshape(6, 2), a)
        d = (np.arange(256) % 251).astype(np.uint8)
        np.testing.assert_array_equal(pipeline.unb64_array(pipeline.b64_array(d, "u1"), "u1"), d)

    # `mirror` flips before extraction: the mirrored query's keypoints sit at the reflected x coordinates.
    def test_mirror_flips_before_extraction(self):
        with tempfile.TemporaryDirectory() as d:
            p = os.path.join(d, "a.png")
            cv2.imwrite(p, cv2.cvtColor(texture(4), cv2.COLOR_RGB2BGR))
            straight, flipped = pipeline.features_payload(p), pipeline.features_payload(p, mirror=True)
            self.assertEqual((straight["w"], straight["h"]), (flipped["w"], flipped["h"]))
            xs = pipeline.unb64_array(straight["pts"], "<f4").reshape(-1, 2)[:, 0]
            xf = pipeline.unb64_array(flipped["pts"], "<f4").reshape(-1, 2)[:, 0]
            self.assertAlmostEqual(float(xs.mean()), straight["w"] - float(xf.mean()), delta=straight["w"] * 0.08)


class FeatureVerifyTest(unittest.TestCase):
    # On the synthetic pairs, verifying from stored features gives the same match/no-match verdict as `verify_one`.
    def test_verdicts_agree_with_verify_one(self):
        agree = {}
        with tempfile.TemporaryDirectory() as d:
            for name, q, c, mirror in cases():
                qp, cp = os.path.join(d, f"{name}_q.png"), os.path.join(d, f"{name}_c.png")
                cv2.imwrite(qp, cv2.cvtColor(q, cv2.COLOR_RGB2BGR))
                cv2.imwrite(cp, cv2.cvtColor(c, cv2.COLOR_RGB2BGR))
                state = pipeline.prepare_query(qp, mirror=mirror)
                legacy = pipeline.verify_one(state, cp)
                feats = pipeline.features_payload(cp)
                feats["path"] = cp
                stored = pipeline.verify_features_one(state, feats)
                agree[name] = (legacy["matched"], stored["matched"])
                if legacy["matched"] and stored["matched"]:
                    self.assertEqual(stored["candidate_size"], legacy["candidate_size"], name)
                    self.assertEqual(stored["mirrored"], legacy["mirrored"], name)
        self.assertEqual({k: v for k, v in agree.items() if v[0] != v[1]}, {}, agree)

    # An exact copy (near-identity fit, ~1e-15 numerical noise) must pass the geometry gate, not read as "outside frame".
    def test_exact_copy_from_stored_features_matches(self):
        with tempfile.TemporaryDirectory() as d:
            for seed in (5, 7, 8):
                p = os.path.join(d, f"{seed}.png")
                cv2.imwrite(p, cv2.cvtColor(texture(seed), cv2.COLOR_RGB2BGR))
                feats = pipeline.features_payload(p)
                feats["path"] = p
                r = pipeline.verify_features_one(pipeline.prepare_query(p), feats)
                self.assertTrue(r["matched"], (seed, r))
                self.assertGreater(r["inliers"], 100)

    # A missing query decode fails every candidate the same way `verify_one` does.
    def test_missing_query(self):
        self.assertEqual(pipeline.verify_features_one(None, {}), {"matched": False, "reason": "decode-failed"})


class ServerJobsTest(unittest.TestCase):
    # The three new job kinds run through `server._run` and return the documented shapes.
    def test_jobs(self):
        import server

        with tempfile.TemporaryDirectory() as d:
            paths = []
            for i, seed in enumerate((5, 6)):
                p = os.path.join(d, f"{i}.png")
                cv2.imwrite(p, cv2.cvtColor(texture(seed), cv2.COLOR_RGB2BGR))
                paths.append(p)
            ex = server._run(server.Job("extract_features", {"paths": paths + [os.path.join(d, "missing.png")]}, "index"))["results"]
            self.assertEqual([("error" in r) for r in ex], [False, False, True])
            self.assertEqual(ex[2]["error"], "decode-failed")
            self.assertEqual(set(ex[0]), {"path", "w", "h", "n", "pts", "sizes", "angles", "desc"})
            cands = [{k: r[k] for k in ("path", "w", "h", "n", "pts", "desc")} for r in ex[:2]]
            res = server._run(server.Job("verify_features", {"query_path": paths[0], "candidates": cands}, "search"))["results"]
            self.assertEqual([r["path"] for r in res], paths)
            self.assertTrue(res[0]["matched"] and not res[1]["matched"])
            json.dumps(res)

            rng = np.random.default_rng(0)
            desc = np.clip(np.rint(rng.normal(60, 25, (2000, 128))), 0, 255).astype(np.uint8)
            out = server._run(server.Job("train_vocab", {"desc": pipeline.b64_array(desc, "u1"), "n": 2000, "k": 32, "iters": 3}, "index"))
            cent = pipeline.unb64_array(out["centroids"], "<f4").reshape(-1, 128)
            self.assertEqual(cent.shape, (32, 128))
            self.assertTrue(np.isfinite(cent).all())


if __name__ == "__main__":
    unittest.main()
