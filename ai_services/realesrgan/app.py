import asyncio
import os
import time
import sys
import torch
import cv2
import numpy as np
from pathlib import Path
from fastapi import FastAPI, HTTPException
from fastapi.responses import JSONResponse
from pydantic import BaseModel, Field
from typing import Optional
from importlib.metadata import PackageNotFoundError, version as pkg_version

# 既存のブリッジロジックをインポート [2]
import realesrgan_bridge as bridge

# GPU は1基のみ。同時リクエストをシリアライズして CUDA OOM を防ぐ
_GPU_SEMAPHORE: asyncio.Semaphore | None = None

app = FastAPI(
    title="SuperBook AI Upscale Service",
    description="RealESRGAN-based image super-resolution service",
    version="1.0.0"
)

class UpscaleRequest(BaseModel):
    input_path: str = Field(..., description="入力画像の絶対パス（コンテナ内）")
    output_path: str = Field(..., description="出力画像の保存先絶対パス")
    scale: int = Field(2, ge=2, le=4)
    tile: int = Field(400)
    model_name: str = Field("realesrgan-x4plus")
    fp32: bool = Field(False)
    gpu_id: int = Field(0)

@app.on_event("startup")
async def startup_event():
    global _GPU_SEMAPHORE
    _GPU_SEMAPHORE = asyncio.Semaphore(1)
    """起動時ウォームアップ。デフォルトは無効（最終実行速度優先）"""
    if os.getenv("AI_STARTUP_WARMUP", "0").lower() not in {"1", "true", "yes", "on"}:
        print("Startup warmup is disabled (AI_STARTUP_WARMUP!=1).")
        return

    print("Initializing model and warming up GPU...")
    try:
        dummy_img = np.zeros((16, 16, 3), dtype=np.uint8)
        dummy_input = Path("/tmp/warmup_in.png")
        dummy_output = Path("/tmp/warmup_out.png")
        cv2.imwrite(str(dummy_input), dummy_img)
        # ウォームアップ実行
        bridge.upscale_image(input_path=dummy_input, output_path=dummy_output)
        print("Warmup completed successfully.")
    except Exception as e:
        print(f"Warmup failed: {e}")

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

@app.post("/cleanup")
async def cleanup():
    """Clean up cached models and free GPU memory."""
    try:
        cleaned = await asyncio.to_thread(bridge.cleanup_upsampler)
        if cleaned:
            return {"status": "ok", "message": "Upsampler cache cleared and GPU memory freed"}
        return JSONResponse(
            status_code=202,
            content={"status": "busy", "message": "Cleanup deferred: active upscale request(s)"},
        )
    except Exception as e:
        print(f"Cleanup failed: {e}", file=sys.stderr)
        return {"status": "error", "message": str(e)}

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

    try:
        # GPU は1基のみ。セマフォで同時アクセスを1件に制限（CUDA OOM 防止）
        async with _GPU_SEMAPHORE:
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
        return result
    except Exception as e:
        print(f"ERROR during upscale: {e}")
        import traceback
        traceback.print_exc()
        raise HTTPException(status_code=500, detail=str(e))

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8000)
    
    