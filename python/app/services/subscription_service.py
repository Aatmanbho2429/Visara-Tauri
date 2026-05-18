import sys
sys.dont_write_bytecode = True

import requests
from app.config import SUPABASE_EDGE
from app.services.auth_service import get_saved_user_id


def get_plans() -> dict:
    try:
        r = requests.get(f"{SUPABASE_EDGE}/get-plans", timeout=10)
        data = r.json()
        if not data.get("success"):
            return {"success": False, "message": data.get("message", "Failed to fetch plans"), "data": None}
        return {"success": True, "message": "Plans fetched successfully", "data": {"plans": data.get("plans", [])}}
    except requests.exceptions.ConnectionError:
        return {"success": False, "message": "No internet connection.", "data": None}
    except Exception as e:
        return {"success": False, "message": str(e), "data": None}


def create_order(user_id: str, plan_id: str) -> dict:
    try:
        r = requests.post(
            f"{SUPABASE_EDGE}/create-order",
            json={"user_id": user_id, "plan_id": plan_id},
            timeout=15
        )
        data = r.json()
        if not data.get("success"):
            return {"success": False, "message": data.get("message", "Failed to create order"), "data": None}
        return {
            "success": True,
            "message": "Order created",
            "data": {
                "order_id": data["order_id"],
                "amount":   data["amount"],
                "currency": data["currency"],
                "key_id":   data["key_id"],
                "plan":     data["plan"],
                "user":     data["user"],
            }
        }
    except requests.exceptions.ConnectionError:
        return {"success": False, "message": "No internet connection.", "data": None}
    except Exception as e:
        return {"success": False, "message": str(e), "data": None}


def get_user_subscriptions() -> dict:
    user_id = get_saved_user_id()
    if not user_id:
        return {"success": False, "message": "No saved session", "data": None}
    try:
        r = requests.post(
            f"{SUPABASE_EDGE}/get-user-subscriptions",
            json={"user_id": user_id},
            timeout=10
        )
        data = r.json()
        if not data.get("success"):
            return {"success": False, "message": data.get("message", "Failed to fetch history"), "data": None}
        return {"success": True, "message": "Fetched", "data": {"subscriptions": data.get("subscriptions", [])}}
    except requests.exceptions.ConnectionError:
        return {"success": False, "message": "No internet connection.", "data": None}
    except Exception as e:
        return {"success": False, "message": str(e), "data": None}


def verify_payment(razorpay_order_id: str, razorpay_payment_id: str,
                   razorpay_signature: str, user_id: str, plan_id: str) -> dict:
    try:
        r = requests.post(
            f"{SUPABASE_EDGE}/verify-payment",
            json={
                "razorpay_order_id":   razorpay_order_id,
                "razorpay_payment_id": razorpay_payment_id,
                "razorpay_signature":  razorpay_signature,
                "user_id":             user_id,
                "plan_id":             plan_id,
            },
            timeout=15
        )
        data = r.json()
        if not data.get("success"):
            return {"success": False, "message": data.get("message", "Payment verification failed"), "data": None}
        return {
            "success": True,
            "message": data.get("message", "Payment verified"),
            "data": {
                "subscription_status": data.get("subscription_status"),
                "subscription_end":    data.get("subscription_end"),
                "days_remaining":      data.get("days_remaining"),
            }
        }
    except requests.exceptions.ConnectionError:
        return {"success": False, "message": "No internet connection.", "data": None}
    except Exception as e:
        return {"success": False, "message": str(e), "data": None}
