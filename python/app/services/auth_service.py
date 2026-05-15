import sys
sys.dont_write_bytecode = True

import os
import requests
from app.config import SUPABASE_EDGE, TOKEN_FILE
from app.services.license_service import get_device_id


def _load_model(onnx_key: str):
    """Load encrypted ONNX model after successful auth."""
    from app.config import MODEL_ENC_PATH
    import os
    print(f"[auth] Loading model from: {MODEL_ENC_PATH}", flush=True)
    if not os.path.exists(MODEL_ENC_PATH):
        print(f"[auth] ERROR: Model file not found at {MODEL_ENC_PATH}", flush=True)
        return
    try:
        from app.core.embedder import Embedder
        Embedder().set_key(onnx_key)
        print(f"[auth] Model loaded successfully", flush=True)
    except Exception as e:
        print(f"[auth] ERROR loading model: {e}", flush=True)


def login(email: str, password: str) -> dict:
    try:
        print(get_device_id())
        r = requests.post(
            f"{SUPABASE_EDGE}/login-user-test",
            json={"email": email, "password": password, "device_id": get_device_id()},
            timeout=15
        )
        data = r.json()

        if not data.get("success"):
            return {"success": False, "message": data.get("message", "Login failed"), "data": None}

        with open(TOKEN_FILE, "w") as f:
            f.write(data["token"])

        if data.get("onnx_key"):
            _load_model(data["onnx_key"])

        return {
            "success": True,
            "message": data.get("message", "Login successful"),
            "data": {"token": data["token"], "user": data["user"]}
        }

    except requests.exceptions.ConnectionError:
        return {"success": False, "message": "No internet connection. Please connect and try again.", "data": None}
    except Exception as e:
        return {"success": False, "message": f"Login error: {str(e)}", "data": None}


def validate_saved_token() -> dict:
    if not os.path.exists(TOKEN_FILE):
        return {"success": False, "message": "No saved session", "data": None}

    try:
        with open(TOKEN_FILE, "r") as f:
            token = f.read().strip()

        if not token:
            return {"success": False, "message": "No saved session", "data": None}

        r = requests.get(
            f"{SUPABASE_EDGE}/validate-token-test",
            headers={"Authorization": f"Bearer {token}", "x-device-id": get_device_id()},
            timeout=10
        )
        data = r.json()

        if not data.get("valid"):
            os.remove(TOKEN_FILE)
            return {"success": False, "message": data.get("message", "Session expired. Please login again."), "data": None}

        if data.get("onnx_key"):
            _load_model(data["onnx_key"])

        return {"success": True, "message": "Session valid", "data": {"user": data["user"]}}

    except requests.exceptions.ConnectionError:
        return {"success": False, "message": "No internet connection. Please connect to login.", "data": None}
    except Exception as e:
        return {"success": False, "message": str(e), "data": None}


def logout() -> dict:
    if os.path.exists(TOKEN_FILE):
        os.remove(TOKEN_FILE)
    try:
        from app.core.embedder import Embedder
        Embedder().reset()
    except Exception:
        pass
    return {"success": True, "message": "Logged out successfully", "data": None}


def register_request(first_name: str, last_name: str, email: str, password: str,
                     phone_number: str = None, company_name: str = None) -> dict:
    try:
        r = requests.post(
            f"{SUPABASE_EDGE}/register-request",
            json={
                "first_name":   first_name,
                "last_name":    last_name,
                "email":        email,
                "password":     password,
                "phone_number": phone_number,
                "company_name": company_name,
                "device_id":    get_device_id()
            },
            timeout=15
        )
        data = r.json()
        return {"success": data.get("success", False), "message": data.get("message", ""), "data": None}

    except requests.exceptions.ConnectionError:
        return {"success": False, "message": "No internet connection. Please connect and try again.", "data": None}
    except Exception as e:
        return {"success": False, "message": f"Registration error: {str(e)}", "data": None}
