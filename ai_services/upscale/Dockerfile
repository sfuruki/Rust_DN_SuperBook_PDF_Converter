# 1. ベースイメージの指定 (NVIDIA CUDA 11.8 搭載の Ubuntu 22.04)
# RealESRGAN (Torch 1.x) に最適なランタイム環境を提供します [1, 3]
FROM docker.io/nvidia/cuda:11.8.0-runtime-ubuntu22.04

LABEL maintainer="superbook-pdf"
LABEL description="SuperBook AI Upscale Service (RealESRGAN)"
LABEL component="upscale-service"

ENV DEBIAN_FRONTEND=noninteractive
ENV TZ=Asia/Tokyo

# 2. システム依存パッケージのインストール
# OpenCV (libgl1) や Python、モデル取得用の curl を含めます [1, 2]
RUN apt-get update && apt-get install -y --no-install-recommends \
    python3 \
    python3-venv \
    libgl1-mesa-glx \
    libglib2.0-0 \
    curl \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# 3. 高速パッケージマネージャー uv の導入 [1, 2]
RUN curl -LsSf https://astral.sh/uv/install.sh | sh
ENV PATH="/root/.local/bin:$PATH"

# 4. アプリケーションディレクトリの作成
WORKDIR /app

# 5. 依存関係のインストール
# FastAPI サーバーと RealESRGAN 関連のライブラリをインストールします [4-6]
ENV AI_VENV=/app/venv
RUN uv venv $AI_VENV --python python3.10 \
    && VIRTUAL_ENV=$AI_VENV uv pip install \
        fastapi \
        uvicorn \
        pydantic \
        realesrgan \
        basicsr \
        opencv-python \
        "numpy<2" \
    && VIRTUAL_ENV=$AI_VENV uv pip install --force-reinstall \
        torch==2.0.1+cu118 torchvision==0.15.2+cu118 \
        --index-url https://download.pytorch.org/whl/cu118

# 6. ソースコードのコピー
# 作成した app.py と既存の bridge スクリプトをコンテナ内に配置します [4]
COPY app.py realesrgan_bridge.py /app/

# 7. 共有データディレクトリの準備
# 4層構成に基づき /data/work をマウントしてゼロコピーを実現します [3, 7]
RUN mkdir -p /data/work
ENV SUPERBOOK_WORK_DIR=/data/work

# 8. 実行設定
ENV NVIDIA_VISIBLE_DEVICES=all
ENV NVIDIA_DRIVER_CAPABILITIES=compute,utility
ENV PATH="$AI_VENV/bin:$PATH"

# FastAPI のポートを公開
EXPOSE 8000

# サービスの起動
# uvicorn を使用して API サーバーを立ち上げます [4]
CMD ["uvicorn", "app:app", "--host", "0.0.0.0", "--port", "8000"]
