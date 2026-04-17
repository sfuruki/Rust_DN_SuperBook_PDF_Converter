import os
import time
import torch
import cv2
import numpy as np
from pathlib import Path
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel, Field
from typing import Optional

# 既存のブリッジロジックをインポート [2]
import realesrgan_bridge as bridge

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
async def warmup():
    """起動時にダミー画像を推論し、モデルをGPUメモリにロードする [1, 3]"""
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
    return {
        "service": "realesrgan-upscale",
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

    input_p = Path(req.input_path)
    output_p = Path(req.output_path)

    if not input_p.exists():
        raise HTTPException(status_code=404, detail=f"Input file not found: {req.input_path}")

    try:
        # 変換後のモデル名を使用してブリッジを呼び出す
        result = bridge.upscale_image(
            input_path=input_p,
            output_path=output_p,
            scale=req.scale,
            tile=req.tile,
            model_name=effective_model_name,
            gpu_id=req.gpu_id,
            fp32=req.fp32
        )
        return result
    except Exception as e:
        print(f"ERROR during upscale: {e}")
        raise HTTPException(status_code=500, detail=str(e))

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8000)
    
    