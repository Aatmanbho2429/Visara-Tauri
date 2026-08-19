from pydantic import BaseModel, Field


class SearchRequest(BaseModel):
    query_image: str = Field(..., description="Absolute path to the image being searched for")
    folder_path: str = Field(..., description="Absolute path to the indexed folder to search in")
    top_k:       int = Field(50, ge=1, le=500, description="Maximum number of matches to return")
