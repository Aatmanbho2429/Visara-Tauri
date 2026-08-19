from fastapi import APIRouter
from app.api.v1.search.schemas import SearchRequest
from app.core.progress import get_progress
from app.services import search_service

router = APIRouter(prefix="/search", tags=["search"])


@router.post("")
async def search(req: SearchRequest):
    return search_service.search(req.query_image, req.folder_path, req.top_k)


@router.get("/progress")
async def progress():
    return {"success": True, "message": "", "data": get_progress()}
