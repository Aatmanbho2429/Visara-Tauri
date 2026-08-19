import threading
import numpy as np
import onnxruntime as ort
from cryptography.fernet import Fernet
from app.config import MODEL_ENC_PATH, CLIP_MEAN, CLIP_STD


class Embedder:
    """Singleton ONNX inference session."""
    _instance  = None
    _init_lock = threading.Lock()

    def __new__(cls):
        if cls._instance is None:
            with cls._init_lock:
                if cls._instance is None:
                    obj = super().__new__(cls)
                    obj._ready = False
                    cls._instance = obj
        return cls._instance

    def __init__(self):
        pass  # waits for set_key() after login

    def set_key(self, key: str):
        """Called after successful login with decryption key from Supabase."""
        with self._init_lock:
            if self._ready:
                return
            self._setup(key)
            self._ready = True

    def _setup(self, key: str):
        with open(MODEL_ENC_PATH, "rb") as f:
            encrypted_bytes = f.read()

        fernet      = Fernet(key.encode())
        model_bytes = fernet.decrypt(encrypted_bytes)

        providers = (
            ["CUDAExecutionProvider", "CPUExecutionProvider"]
            if "CUDAExecutionProvider" in ort.get_available_providers()
            else ["CPUExecutionProvider"]
        )
        opts = ort.SessionOptions()
        opts.execution_mode           = ort.ExecutionMode.ORT_PARALLEL
        opts.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
        opts.inter_op_num_threads     = 4
        opts.intra_op_num_threads     = 4

        self._session  = ort.InferenceSession(model_bytes, opts, providers)
        self._in_name  = self._session.get_inputs()[0].name
        self._out_name = self._session.get_outputs()[0].name
        self._lock     = threading.Lock()
        self._mean     = np.array(CLIP_MEAN, dtype=np.float32)
        self._std      = np.array(CLIP_STD,  dtype=np.float32)

    @property
    def is_ready(self) -> bool:
        return self._ready

    def reset(self):
        """Unload model from memory on logout."""
        with self._init_lock:
            self._ready        = False
            self._session      = None
            Embedder._instance = None

    def embed_batch(self, batch: np.ndarray) -> np.ndarray:
        """batch: (N,3,224,224) → returns (N, EMB_DIM) L2-normalized"""
        if not self._ready:
            raise RuntimeError("Model not loaded — login required")

        with self._lock:
            raw = self._session.run([self._out_name], {self._in_name: batch})[0]

        # Some exported models hardcode batch=1 in output shape
        if raw.shape[0] == 1 and batch.shape[0] > 1:
            results = []
            for i in range(batch.shape[0]):
                with self._lock:
                    out = self._session.run(
                        [self._out_name], {self._in_name: batch[i:i+1]}
                    )[0]
                results.append(out[0])
            raw = np.stack(results)

        norms = np.linalg.norm(raw, axis=1, keepdims=True)
        return (raw / norms).astype(np.float32)

    @property
    def mean(self) -> np.ndarray:
        return self._mean

    @property
    def std(self) -> np.ndarray:
        return self._std
