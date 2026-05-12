from pydantic import BaseModel, EmailStr
from typing import Optional


class LoginRequest(BaseModel):
    email:    EmailStr
    password: str


class RegisterRequest(BaseModel):
    first_name:   str
    last_name:    str
    email:        EmailStr
    password:     str
    phone_number: Optional[str] = None
    company_name: Optional[str] = None
