import os
import time
import torch
from pathlib import Path
from typing import Optional, List, Dict, Any
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel, Field

# 既存のブリッジロジックをインポート（フォルダ移動済み前提）
import yomitoku_bridge as bridge

app = FastAPI(
    title="SuperBook AI OCR Service",
    description="YomiToku-based Japanese AI-OCR service",
    version="1.0.0"
)

# --- リクエストモデルの定義 ---
class OcrRequest(BaseModel):
    input_path: str = Field(..., description="入力画像の絶対パス（コンテナ内）")
    gpu_id: Optional[int] = Field(0, description="使用するGPUデバイスID")
    confidence: float = Field(0.5, ge=0.0, le=1.0, description="確信度の閾値")
    format: str = Field("json", description="出力形式 (json, text, markdown)")

# --- 起動時のウォームアップ (設計案より) ---
@app.on_event("startup")
async def warmup():
    """
    サーバー起動時にダミー画像を推論し、OCRモデルをGPUメモリにロードする。
    これにより、実際の処理リクエスト時の「Cold Start」遅延を防止します。
    """
    print("Initializing YomiToku model and warming up GPU...")
    try:
        # 128x128の白紙ダミー画像を作成して推論
        import numpy as np
        import cv2
        dummy_img_path = Path("/tmp/ocr_warmup.png")
        dummy_img = np.full((128, 128, 3), 255, dtype=np.uint8)
        cv2.imwrite(str(dummy_img_path), dummy_img)
        
        # 最小構成で一度実行
        bridge.process_image(
            dummy_img_path,
            gpu_id=0 if torch.cuda.is_available() else None,
            confidence_threshold=0.5
        )
        print("OCR Warmup completed successfully.")
    except Exception as e:
        print(f"OCR Warmup failed (non-critical): {e}")

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
    return {
        "service": "yomitoku-ocr",
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

    try:
        # 既存のブリッジロジックを使用して画像処理を実行 [3]
        result = bridge.process_image(
            input_p,
            gpu_id=req.gpu_id,
            confidence_threshold=req.confidence
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
        raise HTTPException(status_code=500, detail=str(e))
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))

if __name__ == "__main__":
    import uvicorn
    # コンテナ内待受のため 0.0.0.0 を使用
    uvicorn.run(app, host="0.0.0.0", port=8000)
    