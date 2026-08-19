import numpy as np
import io
import struct
from PIL import Image
from concurrent.futures import ThreadPoolExecutor
from psd_tools import PSDImage
from app.config import NUM_WORKERS
from app.core.embedder import Embedder


def _read_psb_thumbnail(path: str):
    """Extract embedded JPEG thumbnail from PSB/PSD without full decode."""
    try:
        with open(path, "rb") as f:
            if f.read(4) != b"8BPS":
                return None
            f.read(2); f.read(6); f.read(2); f.read(4); f.read(4); f.read(4)

            color_mode_len = struct.unpack(">I", f.read(4))[0]
            f.read(color_mode_len)

            resources_len = struct.unpack(">I", f.read(4))[0]
            resources_end = f.tell() + resources_len

            while f.tell() < resources_end:
                if f.read(4) != b"8BIM":
                    break
                res_id   = struct.unpack(">H", f.read(2))[0]
                name_len = struct.unpack("B", f.read(1))[0]
                pad      = 1 if (name_len + 1) % 2 != 0 else 0
                f.read(name_len + pad)
                data_len = struct.unpack(">I", f.read(4))[0]
                data_pos = f.tell()

                if res_id in (1033, 1036):
                    f.read(28)
                    jpeg_len = data_len - 28
                    if jpeg_len > 0:
                        img = Image.open(io.BytesIO(f.read(jpeg_len)))
                        return img.convert("RGB")

                f.seek(data_pos + data_len + (data_len % 2))
    except Exception:
        pass
    return None


def load_image_fast(path: str) -> Image.Image:
    ext = path.lower()

    # ── PSD / PSB ────────────────────────────────────────────────────────
    if ext.endswith((".psd", ".psb")):
        img = _read_psb_thumbnail(path)
        if img is not None:
            return img
        psd = PSDImage.open(path)
        img = psd.topil()
        if img is None:
            img = psd.composite()
        if img is None:
            raise RuntimeError(f"PSD/PSB load failed: {path}")
        return img.convert("RGB")

    # ── TIFF ─────────────────────────────────────────────────────────────
    if ext.endswith((".tif", ".tiff")):
        img = Image.open(path)
        try:
            img.seek(1)
            if max(img.size) <= 1024:
                return img.convert("RGB")
        except Exception:
            pass
        img.seek(0)
        try:
            img.draft("RGB", (512, 512))
        except Exception:
            pass
        return img.convert("RGB")

    # ── Everything else (JPG, PNG, etc.) ─────────────────────────────────
    return Image.open(path).convert("RGB")


def preprocess_single(path: str) -> np.ndarray:
    embedder = Embedder()
    img = load_image_fast(path)
    img = img.resize((224, 224), Image.BILINEAR)
    arr = np.array(img, dtype=np.float32) / 255.0
    arr = (arr - embedder.mean) / embedder.std
    arr = np.transpose(arr, (2, 0, 1))
    return arr


def preprocess_batch_parallel(paths: list) -> tuple:
    results = [None] * len(paths)

    def load_one(args):
        i, path = args
        try:
            results[i] = (preprocess_single(path), path, None)
        except Exception as e:
            results[i] = (None, path, str(e))

    with ThreadPoolExecutor(max_workers=NUM_WORKERS) as ex:
        list(ex.map(load_one, enumerate(paths)))

    batch, valid_paths, failed = [], [], []
    for arr, path, err in results:
        if arr is not None:
            batch.append(arr)
            valid_paths.append(path)
        else:
            failed.append({"file": path, "reason": err})

    if not batch:
        return None, [], failed

    return np.stack(batch).astype(np.float32), valid_paths, failed
