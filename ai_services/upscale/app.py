import os
import time
import torch
import cv2
import numpy as np
from pathlib import Path
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel, Field
from typing import Optional, Dict, Any

# 既存のブリッジロジックを流用（フォルダ移動済み前提）
import realesrgan_bridge as bridge

app = FastAPI(
    title="SuperBook AI Upscale Service",
    description="RealESRGAN-based image super-resolution service",
    version="1.0.0"
)

# --- リクエストモデルの定義 ---
class UpscaleRequest(BaseModel):
    input_path: str = Field(..., description="入力画像の絶対パス（コンテナ内）")
    output_path: str = Field(..., description="出力画像の保存先絶対パス")
    scale: int = Field(2, ge=2, le=4, description="拡大倍率 (2 or 4)")
    tile: int = Field(400, description="タイルサイズ（GPUメモリ節約用）")
    model_name: str = Field("realesrgan-x4plus", description="使用するモデル名")
    fp32: bool = Field(False, description="FP32精度を使用するか（Trueで低速だが高精度）")
    gpu_id: int = Field(0, description="使用するGPUデバイスID")

# --- 起動時のウォームアップ (提案2) ---
@app.on_event("startup")
async def warmup():
    """
    サーバー起動時にダミー画像を推論し、モデルをGPUメモリにロードする。
    これにより、初回の処理リクエスト時の遅延（Cold Start）を防止します。 [2]
    """
    print("Initializing model and warming up GPU...")
    try:
        # 16x16のダミー画像を生成
        dummy_img = np.zeros((16, 16, 3), dtype=np.uint8)
        dummy_input = Path("/tmp/warmup_in.png")
        dummy_output = Path("/tmp/warmup_out.png")
        cv2.imwrite(str(dummy_input), dummy_img)
        
        # 最小設定で一度実行
        bridge.upscale_image(
            dummy_input, 
            dummy_output, 
            scale=2, 
            tile=128, 
            model_name="realesrgan-x4plus"
        )
        print("Warmup completed successfully.")
    except Exception as e:
        print(f"Warmup failed (this is non-critical if GPU is busy): {e}")

# --- API エンドポイント ---

@app.get("/health")
async def health_check():
    """サービスの生存確認用"""
    return {"status": "healthy", "timestamp": time.time()}

@app.get("/version")
async def get_version():
    """
    システム情報を返却。Rust Core側のハンドシェイクに使用されます。 [2]
    """
    return {
        "service": "realesrgan-upscale",
        "torch_version": torch.__version__,
        "cuda_available": torch.cuda.is_available(),
        "device": torch.cuda.get_device_name(0) if torch.cuda.is_available() else "cpu",
        "vram_total": torch.cuda.get_device_properties(0).total_memory // (1024**2) if torch.cuda.is_available() else 0
    }

@app.post("/upscale")
async def upscale(req: UpscaleRequest):
    """
    画像の超解像処理を実行。
    ゼロコピー設計により、画像データではなく共有ボリューム上のパスを受け取ります。 [2, 4]
    """
    input_p = Path(req.input_path)
    output_p = Path(req.output_path)

    # パスの存在確認
    if not input_p.exists():
        raise HTTPException(status_code=404, detail=f"Input file not found: {req.input_path}")

    # 出力ディレクトリの準備
    output_p.parent.mkdir(parents=True, exist_ok=True)

    try:
        # 既存のブリッジロジックを実行
        result = bridge.upscale_image(
            input_p,
            output_p,
            scale=req.scale,
            tile=req.tile,
            gpu_id=req.gpu_id,
            model_name=req.model_name,
            fp32=req.fp32
        )

        if result.get("exit_code") != 0:
            raise Exception(result.get("error", "Unknown error in upscale logic"))

        return result

    except RuntimeError as e:
        if "out of memory" in str(e).lower():
            raise HTTPException(status_code=507, detail="GPU Out of Memory. Try reducing tile size.")
        raise HTTPException(status_code=500, detail=str(e))
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))

if __name__ == "__main__":
    import uvicorn
    # コンテナ内では 0.0.0.0 でバインド必須
    uvicorn.run(app, host="0.0.0.0", port=8000)
    