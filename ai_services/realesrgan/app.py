import asyncio
import os
import time
import sys
import threading
import torch
from pathlib import Path
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel, Field
from typing import Optional
from importlib.metadata import PackageNotFoundError, version as pkg_version

# 既存のブリッジロジックをインポート [2]
import realesrgan_bridge as bridge

app = FastAPI(
    title="SuperBook AI Upscale Service",
    description="RealESRGAN-based image super-resolution service",
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

class UpscaleRequest(BaseModel):
    input_path: str = Field(..., description="入力画像の絶対パス（コンテナ内）")
    output_path: str = Field(..., description="出力画像の保存先絶対パス")
    scale: int = Field(2, ge=2, le=4)
    tile: int = Field(256)
    model_name: str = Field("realesrgan-x4plus")
    fp32: bool = Field(False)
    gpu_id: int = Field(0)

@app.on_event("startup")
async def startup_event():
    if os.getenv("AI_STARTUP_WARMUP", "0").lower() not in {"1", "true", "yes", "on"}:
        print("Startup warmup is disabled (AI_STARTUP_WARMUP!=1).")
        return

    print("Loading RealESRGAN model weights into GPU memory...")
    try:
        # Model load only — no dummy inference
        await asyncio.to_thread(
            bridge.get_or_create_upsampler,
            "realesrgan-x4plus",
            2,
            256,
            0,
            False,
        )
        print("Model loaded successfully.")
    except Exception as e:
        print(f"Model load failed (non-critical): {e}")

@app.get("/status")
async def get_status():
    """GPU memory status and model load state."""
    runtime = bridge.get_runtime_stats()
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
        "service": "realesrgan-upscale",
        "status": "running",
        "active_requests": _get_active_requests(),
        "active_inference": runtime.get("active_inference", 0),
        "measured_inference_mb": runtime.get("measured_inference_mb", 0.0),
        "measured_inference_samples": runtime.get("measured_inference_samples", 0),
        "upsampler_pool_size": runtime.get("upsampler_pool_size", 1),
        "upsampler_slots": runtime.get("upsampler_slots", 0),
        "gpu_memory_total": memory_total_mb,
        "gpu_memory_used": memory_reserved_mb,
        "gpu_memory_free": memory_free_mb,
        "gpu": gpu_info,
        "cuda_available": torch.cuda.is_available(),
    }
@app.get("/version")
async def get_version():
    """システム情報を返却。Rust Core側のハンドシェイクに使用 [1, 4]"""
    try:
        realesrgan_version = pkg_version("realesrgan")
    except PackageNotFoundError:
        realesrgan_version = "unknown"

    return {
        "service": "realesrgan-upscale",
        "service_version": realesrgan_version,
        "python_version": sys.version.split()[0],
        "torch_version": torch.__version__,
        "cuda_available": torch.cuda.is_available(),
        "device": torch.cuda.get_device_name(0) if torch.cuda.is_available() else "cpu"
    }

@app.post("/upscale")
async def upscale(req: UpscaleRequest):
    """
    画像の超解像処理を実行。
    Rust Coreからのモデル名表記の揺れをここで吸収する [1]。
    """
    # --- モデル名のマッピング処理 (修正の肝) ---
    model_mapping = {
        "RealESRGAN_x4plus": "realesrgan-x4plus",
        "RealESRGAN_x4plus_anime": "realesrgan-x4plus-anime",
        "RealESRGAN_x2plus": "realesrgan-x2plus",
        "X4Plus": "realesrgan-x4plus",
        "X2Plus": "realesrgan-x2plus"
    }
    
    # Rust Coreから届いた名前を変換。未定義ならそのまま使用
    effective_model_name = model_mapping.get(req.model_name, req.model_name)
    
    print(f"DEBUG: Received: {req.model_name} -> Mapped to: {effective_model_name}")
    print(f"DEBUG: Request payload: input_path={req.input_path}, output_path={req.output_path}, scale={req.scale}")

    input_p = Path(req.input_path)
    output_p = Path(req.output_path)

    print(f"DEBUG: Checking input file: {input_p.absolute()}, exists={input_p.exists()}")
    if not input_p.exists():
        print(f"ERROR: Input file not found: {req.input_path}")
        raise HTTPException(status_code=400, detail=f"Input file not found: {req.input_path}")

    _inc_active_requests()
    try:
        result = await asyncio.to_thread(
            bridge.upscale_image,
            input_path=input_p,
            output_path=output_p,
            scale=req.scale,
            tile=req.tile,
            model_name=effective_model_name,
            gpu_id=req.gpu_id,
            fp32=req.fp32
        )
        print(f"DEBUG: Upscale completed: {result}")

        # Never report AI failure as HTTP 200, or Rust side may treat it as success.
        exit_code = int(result.get("exit_code", 1))
        if exit_code != 0:
            error_message = result.get("error", "Upscale failed")
            status_code = 500
            if exit_code == 3:
                status_code = 400
            elif exit_code == 6:
                status_code = 507
            elif exit_code == 5:
                status_code = 503
            raise HTTPException(
                status_code=status_code,
                detail=f"RealESRGAN failed (exit_code={exit_code}): {error_message}",
            )

        return result
    except Exception as e:
        print(f"ERROR during upscale: {e}")
        import traceback
        traceback.print_exc()
        raise HTTPException(status_code=500, detail=str(e))
    finally:
        _dec_active_requests()

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8000)
    
    