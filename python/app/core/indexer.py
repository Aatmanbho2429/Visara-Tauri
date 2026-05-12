import sys
sys.dont_write_bytecode = True

import os
import numpy as np
import faiss
from app.config import INDEX_PATH, FAISS_DIR, EMB_DIM


def load_index() -> faiss.Index:
    os.makedirs(FAISS_DIR, exist_ok=True)
    if os.path.exists(INDEX_PATH):
        try:
            return faiss.read_index(INDEX_PATH)
        except Exception as e:
            raise RuntimeError(f"Failed to load FAISS index: {e}")
    return faiss.IndexIDMap(faiss.IndexFlatIP(EMB_DIM))


def save_index(index: faiss.Index):
    os.makedirs(FAISS_DIR, exist_ok=True)
    faiss.write_index(index, INDEX_PATH)


def add_embedding(index: faiss.Index, emb: np.ndarray, faiss_id: int):
    index.add_with_ids(emb.reshape(1, -1), np.array([faiss_id]))


def remove_embeddings(index: faiss.Index, faiss_ids: list):
    if faiss_ids:
        index.remove_ids(np.array(faiss_ids))


def search_index(index: faiss.Index, query: np.ndarray, top_k: int):
    D, I = index.search(query.reshape(1, -1), top_k)
    return D[0], I[0]
