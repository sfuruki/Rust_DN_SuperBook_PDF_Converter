# YomiToku OCR Service
FROM docker.io/nvidia/cuda:12.1.1-runtime-ubuntu22.04

ENV DEBIAN_FRONTEND=noninteractive
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    python3 python3-pip libgl1-mesa-glx libglib2.0-0 curl ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# 高速インストーラー uv を導入 [前に解決した方針]
RUN curl -LsSf https://astral.sh/uv/install.sh | sh
ENV PATH="/root/.local/bin:$PATH"

COPY requirements.txt .
# 前に解決した方針: Torch を cu121 インデックスから先行インストール
RUN uv pip install --system --no-cache \
    torch>=2.0.0 torchvision>=0.15.0 \
    --index-url https://download.pytorch.org/whl/cu121 && \
    uv pip install --system --no-cache -r requirements.txt

COPY . .

# ゼロコピー用のディレクトリ作成
RUN mkdir -p /data/work

EXPOSE 8000
CMD ["uvicorn", "app:app", "--host", "0.0.0.0", "--port", "8000", "--workers", "1"]