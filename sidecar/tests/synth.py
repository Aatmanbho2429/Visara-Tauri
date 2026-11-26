# Seeded synthetic textures shared by the sidecar tests and the eval-harness smoke test.
import cv2
import numpy as np


# A textured RGB image: blurred seeded noise plus a few shapes, so SIFT and the DCT hash both have structure to find.
def texture(seed, h=360, w=480):
    rng = np.random.default_rng(seed)
    base = (rng.random((h // 12, w // 12, 3)) * 255).astype(np.uint8)
    img = cv2.resize(base, (w, h), interpolation=cv2.INTER_CUBIC)
    fine = (rng.random((h, w, 3)) * 60).astype(np.uint8)
    img = cv2.add(img, fine)
    for _ in range(8):
        c = tuple(int(x) for x in rng.integers(0, 255, 3))
        x, y = int(rng.integers(20, w - 20)), int(rng.integers(20, h - 20))
        if rng.random() < 0.5:
            cv2.circle(img, (x, y), int(rng.integers(10, 50)), c, -1)
        else:
            cv2.rectangle(img, (x, y), (min(w - 1, x + int(rng.integers(20, 90))), min(h - 1, y + int(rng.integers(20, 90)))), c, int(rng.integers(2, 8)))
    return img
