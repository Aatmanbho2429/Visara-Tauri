from fastapi import APIRouter
from app.services.subscription_service import get_plans

router = APIRouter(prefix="/subscription", tags=["subscription"])


@router.get("/plans")
def plans():
    return get_plans()
