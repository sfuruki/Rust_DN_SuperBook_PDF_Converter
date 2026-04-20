# ai_services/realesrgan/Dockerfile

FROM docker.io/nvidia/cuda:11.8.0-runtime-ubuntu22.04
ENV DEBIAN_FRONTEND=noninteractive
WORKDIR /app/realesrgan

# 1. システム依存パッケージのインストール [4, 8]
RUN apt-get update && apt-get install -y --no-install-recommends \
    python3 python3-pip libgl1-mesa-glx libglib2.0-0 curl ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# 2. 元ソースで使われていた高速インストーラー 'uv' を導入 [4]
RUN curl -LsSf https://astral.sh/uv/install.sh | sh
ENV PATH="/root/.local/bin:$PATH"

# 3. 依存関係のコピー
COPY requirements.txt .

# 4. 【最重要】前に解決した方針に完全に従う [5]
# 順序: Torch先行インストール → realesrgan → requirements.txt
# 注: CUDA 11.8 + Torch 2.0.1 を使用（YomiToku と別コンテナで物理的に分離）[構築方針]
RUN uv pip install --system --no-cache \
    torch==2.0.1+cu118 torchvision==0.15.2+cu118 \
    --index-url https://download.pytorch.org/whl/cu118 && \
    uv pip install --system --no-cache realesrgan && \
    uv pip install --system --no-cache -r requirements.txt

# 5. ソースコードのコピーとディレクトリ作成 [8]
COPY . .
RUN mkdir -p /data/work

EXPOSE 8000
CMD ["uvicorn", "app:app", "--host", "0.0.0.0", "--port", "8000", "--workers", "1"]
