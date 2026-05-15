import sys
sys.dont_write_bytecode = True

import requests
from app.config import SUPABASE_EDGE


def get_plans() -> dict:
    try:
        r = requests.get(
            f"{SUPABASE_EDGE}/get-plans",
            timeout=10
        )
        data = r.json()

        if not data.get("success"):
            return {"success": False, "message": data.get("message", "Failed to fetch plans"), "data": None}

        return {
            "success": True,
            "message": "Plans fetched successfully",
            "data": {"plans": data.get("plans", [])}
        }

    except requests.exceptions.ConnectionError:
        return {"success": False, "message": "No internet connection.", "data": None}
    except Exception as e:
        return {"success": False, "message": str(e), "data": None}
