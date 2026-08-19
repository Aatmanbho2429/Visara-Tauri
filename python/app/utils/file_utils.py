import os
import hashlib
from app.config import IMAGE_EXTENSIONS, HASH_BYTES


def fast_hash(path: str) -> str:
    h = hashlib.sha256()
    h.update(str(os.path.getsize(path)).encode())
    with open(path, "rb") as f:
        h.update(f.read(HASH_BYTES))
    return h.hexdigest()

_SKIP_FOLDERS = {"__macosx"}

def scan_images(folder: str):
    for root, dirs, files in os.walk(folder):
        dirs[:] = [d for d in dirs if d.lower() not in _SKIP_FOLDERS]
        for f in files:
            if f.lower().endswith(IMAGE_EXTENSIONS):
                yield os.path.normpath(os.path.join(root, f))
