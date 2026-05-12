from pydantic import BaseModel


class SearchRequest(BaseModel):
    image_path:  str
    folder_path: str
    top_k:       int = 50
