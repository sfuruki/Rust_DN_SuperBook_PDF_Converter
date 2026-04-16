# RealESRGAN Upscale Service
FROM docker.io/nvidia/cuda:11.8.0-runtime-ubuntu22.04

ENV DEBIAN_FRONTEND=noninteractive
WORKDIR /app

# システム依存パッケージ（OpenCV用など）
RUN apt-get update && apt-get install -y --no-install-recommends \
    python3 python3-pip libgl1-mesa-glx libglib2.0-0 curl && \
    rm -rf /var/lib/apt/lists/*

# 依存関係のインストール
COPY requirements.txt .
RUN pip3 install --no-cache-dir wheel setuptools && \
    pip3 install --no-cache-dir -r requirements.txt && \
    pip3 install --no-cache-dir basicsr==1.4.2 --no-deps

# ソースコードのコピー
COPY . .

# ゼロコピー用のディレクトリ作成
RUN mkdir -p /data/work

EXPOSE 8000
CMD ["uvicorn", "app:app", "--host", "0.0.0.0", "--port", "8000"]
