# Smoke test for the eval harness on 30 seeded synthetic textures: report shape, and rot90/resize must verify every time.
import json
import os
import subprocess
import sys
import tempfile
import unittest

import cv2

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

from synth import texture  # noqa: E402

TOOL = os.path.join(HERE, "..", "tools", "eval_exact_match.py")


class EvalSmokeTest(unittest.TestCase):
    def test_report_shape_and_easy_transforms(self):
        with tempfile.TemporaryDirectory() as d:
            lib = os.path.join(d, "lib")
            os.makedirs(lib)
            for i in range(30):
                cv2.imwrite(os.path.join(lib, f"t{i:02d}.png"), cv2.cvtColor(texture(500 + i), cv2.COLOR_RGB2BGR))
            out = os.path.join(d, "report.json")
            proc = subprocess.run([sys.executable, TOOL, "--library", lib, "--n", "30", "--negatives", "5", "--seed", "0", "--out", out],
                                  capture_output=True, text=True, timeout=600)
            self.assertEqual(proc.returncode, 0, proc.stderr)
            with open(out) as f:
                report = json.load(f)
            self.assertEqual(report["config"]["detector"], "sift")
            self.assertEqual(len(report["transforms"]), 12)
            keys = {"n", "true_positive_rate", "false_copy_rate", "verify_ms_median", "verify_ms_p95", "feature_bytes_mean",
                    "phash_source_median", "phash_negative_median", "phash_negative_min"}
            for name, r in report["transforms"].items():
                self.assertEqual(set(r), keys, name)
                self.assertEqual(r["n"], 30, name)
                self.assertEqual(r["false_copy_rate"], 0.0, f"{name}: a negative passed the copy thresholds")
            for name in ("rot90", "resize_0.5x"):
                self.assertEqual(report["transforms"][name]["true_positive_rate"], 1.0, name)

    # Any detector other than sift exits with a clear message instead of a traceback.
    def test_other_detectors_are_not_implemented(self):
        proc = subprocess.run([sys.executable, TOOL, "--library", HERE, "--detector", "xfeat"], capture_output=True, text=True)
        self.assertEqual(proc.returncode, 2)
        self.assertIn("not implemented", proc.stderr)


if __name__ == "__main__":
    unittest.main()
