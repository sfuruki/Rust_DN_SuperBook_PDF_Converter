import os
import time
import asyncio
import traceback
import sys
import threading
import torch
from pathlib import Path
from typing import Optional, List, Dict, Any
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel, Field
from importlib.metadata import PackageNotFoundError, version as pkg_version

# 既存のブリッジロジックをインポート（フォルダ移動済み前提）
import yomitoku_bridge as bridge

app = FastAPI(
    title="SuperBook AI OCR Service",
    description="YomiToku-based Japanese AI-OCR service",
    version="1.0.0"
)

_active_requests = 0
_active_requests_lock = threading.Lock()


def _inc_active_requests():
    global _active_requests
    with _active_requests_lock:
        _active_requests += 1


def _dec_active_requests():
    global _active_requests
    with _active_requests_lock:
        _active_requests = max(0, _active_requests - 1)


def _get_active_requests() -> int:
    with _active_requests_lock:
        return _active_requests

# --- リクエストモデルの定義 ---
class OcrRequest(BaseModel):
    input_path: str = Field(..., description="入力画像の絶対パス（コンテナ内）")
    gpu_id: Optional[int] = Field(0, description="使用するGPUデバイスID")
    language: str = Field("jpn", description="OCR language (YomiToku currently supports Japanese)")
    confidence: float = Field(0.5, ge=0.0, le=1.0, description="確信度の閾値")
    format: str = Field("json", description="出力形式 (json, text, markdown)")

@app.on_event("startup")
async def warmup():
    """Startup warmup: model load only, no dummy inference."""
    if os.getenv("AI_STARTUP_WARMUP", "0").lower() not in {"1", "true", "yes", "on"}:
        print("Startup warmup is disabled (AI_STARTUP_WARMUP!=1).")
        return

    print("Loading YomiToku model weights into GPU memory...")
    try:
        device = f"cuda:0" if torch.cuda.is_available() else "cpu"
        # Model load only — no dummy inference
        await asyncio.to_thread(bridge.get_or_create_analyzer, device)
        print("YomiToku model loaded successfully.")
    except Exception as e:
        print(f"YomiToku model load failed (non-critical): {e}")

@app.get("/status")
async def get_status():
    """GPU memory status and model load state."""
    gpu_info = {}
    memory_allocated_mb = 0.0
    memory_reserved_mb = 0.0
    memory_total_mb = 0.0
    memory_free_mb = 0.0
    if torch.cuda.is_available():
        try:
            memory_allocated_mb = round(torch.cuda.memory_allocated(0) / 1024**2, 1)
            memory_reserved_mb = round(torch.cuda.memory_reserved(0) / 1024**2, 1)
            memory_total_mb = round(torch.cuda.get_device_properties(0).total_memory / 1024**2, 1)
            memory_free_mb = round(max(0.0, memory_total_mb - memory_reserved_mb), 1)
            gpu_info = {
                "device": torch.cuda.get_device_name(0),
                "memory_allocated_mb": memory_allocated_mb,
                "memory_reserved_mb": memory_reserved_mb,
                "memory_total_mb": memory_total_mb,
                "memory_free_mb": memory_free_mb,
            }
        except Exception:
            gpu_info = {"error": "Failed to query GPU memory"}
    return {
        "service": "yomitoku-ocr",
        "status": "running",
        "active_requests": _get_active_requests(),
        "gpu_memory_total": memory_total_mb,
        "gpu_memory_used": memory_reserved_mb,
        "gpu_memory_free": memory_free_mb,
        "gpu": gpu_info,
        "cuda_available": torch.cuda.is_available(),
    }

# --- API エンドポイント ---

@app.get("/health")
async def health_check():
    """サービスの生存確認用"""
    return {"status": "healthy", "timestamp": time.time()}

@app.get("/version")
async def get_version():
    """
    システム情報を返却。Rust Core側のハンドシェイクに使用されます [2]。
    """
    try:
        yomitoku_version = pkg_version("yomitoku")
    except PackageNotFoundError:
        yomitoku_version = "unknown"

    return {
        "service": "yomitoku-ocr",
        "service_version": yomitoku_version,
        "python_version": sys.version.split()[0],
        "torch_version": torch.__version__ if torch else "not_installed",
        "cuda_available": torch.cuda.is_available() if torch else False,
        "device": torch.cuda.get_device_name(0) if (torch and torch.cuda.is_available()) else "cpu",
        "yomitoku_available": bridge.YOMITOKU_AVAILABLE,
        "vram_total_gb": torch.cuda.get_device_properties(0).total_memory / 1e9 if (torch and torch.cuda.is_available()) else 0
    }

@app.post("/ocr")
async def perform_ocr(req: OcrRequest):
    """
    画像のOCR処理を実行。
    ゼロコピー設計により、パス文字列のみをやり取りします [2]。
    """
    input_p = Path(req.input_path)

    # パスの存在確認
    if not input_p.exists():
        raise HTTPException(status_code=404, detail=f"Input file not found: {req.input_path}")

    normalized_language = req.language.strip().lower()
    if normalized_language not in {"jpn", "ja", "ja-jp", "japanese"}:
        raise HTTPException(
            status_code=400,
            detail=f"Unsupported OCR language for YomiToku service: {req.language}",
        )

    _inc_active_requests()
    try:
        # 既存のブリッジロジックを使用して画像処理を実行 [3]
        result = await asyncio.to_thread(
            bridge.process_image,
            input_p,
            "json",
            req.gpu_id,
            req.confidence,
        )

        if result.get("exit_code") != 0:
            raise Exception(result.get("error", "Unknown error in YomiToku logic"))

        # 指定されたフォーマットで返却準備 [4]
        if req.format != "json":
            formatted_text = bridge.format_output(result, req.format)
            return {"raw_result": result, "formatted": formatted_text}
        
        # Rust Coreのパースしやすさのため、一貫したキーで返却
        return {
            "status": "success",
            "text_blocks": result.get("text_blocks", []),
            "full_text": result.get("full_text", ""),
            "confidence": result.get("confidence", 0.0),
            "text_direction": result.get("text_direction", "horizontal")
        }

    except RuntimeError as e:
        if "out of memory" in str(e).lower():
            print("CRITICAL: GPU Out of Memory detected.")
            raise HTTPException(status_code=507, detail="GPU Out of Memory in OCR Service.")
        print(f"Runtime Error: {e}")
        traceback.print_exc()
        raise HTTPException(status_code=500, detail=str(e))
    except Exception as e:
        traceback.print_exc()
        raise HTTPException(status_code=500, detail=str(e))
    finally:
        _dec_active_requests()

if __name__ == "__main__":
    import uvicorn
    # コンテナ内待受のため 0.0.0.0 を使用
    uvicorn.run(app, host="0.0.0.0", port=8000)
    