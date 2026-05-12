import sys
sys.dont_write_bytecode = True

from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware
from app.api.v1.router import router as v1_router
from app.core.exceptions import VisaraException, visara_exception_handler

app = FastAPI(title="Visara API", version="1.1.7")

app.add_middleware(
    CORSMiddleware,
    allow_origins=["tauri://localhost", "http://localhost:4200"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

app.add_exception_handler(VisaraException, visara_exception_handler)
app.include_router(v1_router)


@app.get("/health")
async def health():
    return {"status": "ok"}


if __name__ == "__main__":
    import uvicorn
    uvicorn.run("main:app", host="127.0.0.1", port=8765, reload=False)
