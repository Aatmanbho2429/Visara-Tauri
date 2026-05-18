from fastapi import APIRouter
from pydantic import BaseModel
from app.services.subscription_service import get_plans, create_order, verify_payment, get_user_subscriptions

router = APIRouter(prefix="/subscription", tags=["subscription"])


class CreateOrderRequest(BaseModel):
    user_id: str
    plan_id: str


class VerifyPaymentRequest(BaseModel):
    razorpay_order_id:   str
    razorpay_payment_id: str
    razorpay_signature:  str
    user_id:             str
    plan_id:             str


@router.get("/plans")
def plans():
    return get_plans()


@router.post("/history")
def history():
    return get_user_subscriptions()


@router.post("/create-order")
def create_order_endpoint(body: CreateOrderRequest):
    return create_order(body.user_id, body.plan_id)


@router.post("/verify-payment")
def verify_payment_endpoint(body: VerifyPaymentRequest):
    return verify_payment(
        body.razorpay_order_id,
        body.razorpay_payment_id,
        body.razorpay_signature,
        body.user_id,
        body.plan_id,
    )
