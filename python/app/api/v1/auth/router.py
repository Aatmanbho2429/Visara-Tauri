from fastapi import APIRouter
from app.api.v1.auth.schemas import LoginRequest, RegisterRequest
from app.services import auth_service

router = APIRouter(prefix="/auth", tags=["auth"])


@router.post("/login")
async def login(req: LoginRequest):
    return auth_service.login(req.email, req.password)


@router.get("/validate-token")
async def validate_token():
    return auth_service.validate_saved_token()


@router.post("/request-access")
async def request_access(req: RegisterRequest):
    return auth_service.register_request(
        first_name=req.first_name,
        last_name=req.last_name,
        email=req.email,
        password=req.password,
        phone_number=req.phone_number,
        company_name=req.company_name
    )
