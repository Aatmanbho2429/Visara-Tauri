import os
import numpy as np
import faiss
from app.config import INDEX_PATH, FAISS_DIR, EMB_DIM


def load_index() -> faiss.Index:
    os.makedirs(FAISS_DIR, exist_ok=True)
    if os.path.exists(INDEX_PATH):
        try:
            return faiss.read_index(INDEX_PATH)
        except Exception:
            return "Error loading faiss.index"
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


def search_index_multi(index: faiss.Index, queries: np.ndarray, top_k: int):
    """
    Search the index with several query vectors at once.

    queries: (N, EMB_DIM) — e.g. one row per orientation of the same image.
    Returns (scores, ids), both (N, top_k). FAISS pads short result rows
    with id -1, which callers must skip.
    """
    queries = np.ascontiguousarray(queries.reshape(-1, EMB_DIM), dtype=np.float32)
    top_k   = max(1, min(top_k, index.ntotal)) if index.ntotal else 1
    return index.search(queries, top_k)
