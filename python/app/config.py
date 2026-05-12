import os
import sys


def get_base_dir():
    if getattr(sys, "frozen", False):
        return sys._MEIPASS
    return os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def get_data_dir():
    if getattr(sys, "frozen", False):
        return os.path.dirname(sys.executable)
    return os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


BASE_DIR  = get_base_dir()
DATA_DIR  = get_data_dir()

FAISS_DIR  = os.path.join(DATA_DIR, "faiss")
MODEL_PATH = os.path.join(BASE_DIR, "models", "clip_vitb32.onnx")
DB_PATH    = os.path.join(FAISS_DIR, "meta.db")
INDEX_PATH = os.path.join(FAISS_DIR, "index.faiss")

IMAGE_EXTENSIONS          = (".jpg", ".jpeg", ".png", ".tif", ".tiff", ".psd", ".psb")
IMAGE_EXTENSIONS_FOR_FILE = "*.jpg *.jpeg *.png *.tiff *.tif *.psd *.psb"

BATCH_SIZE  = 64
NUM_WORKERS = 8
HASH_BYTES  = 65536
EMB_DIM     = 768

CLIP_MEAN = [0.48145466, 0.4578275,  0.40821073]
CLIP_STD  = [0.26862954, 0.26130258, 0.27577711]

MODEL_ENC_PATH = os.path.join(BASE_DIR, "models", "clip_vitb32.onnx.enc")
TOKEN_FILE     = os.path.join(os.path.expanduser("~"), ".visara_token")

SUPABASE_EDGE = "https://qpxvwdxuhgbthzbcppye.supabase.co/functions/v1"
APP_VERSION   = "1.1.7"
VERSION_URL   = "https://raw.githubusercontent.com/Aatmanbho2429/Visara/main/version.json"
