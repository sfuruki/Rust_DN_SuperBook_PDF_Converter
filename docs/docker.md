# Docker / Podman ガイド

superbook-pdf を Docker / Podman で実行するためのガイドです。

---

## サポートする GPU 環境

| 環境 | Compose ファイル | ベースイメージ |
|------|----------------|--------------|
| NVIDIA GPU | `docker-compose.yml` | `superbook-pdf/Dockerfile` |
| AMD GPU (ROCm) | `docker-compose.rocm.yml` | `docker/backend/Dockerfile.rocm` |
| CPU のみ | `docker-compose.cpu.yml` | `docker/backend/Dockerfile.cpu` |

---

## クイックスタート

### Web UI の起動

```bash
# NVIDIA GPU (デフォルト)
docker compose up -d

# AMD GPU (ROCm)
docker compose -f docker-compose.yml -f docker-compose.rocm.yml up -d

# CPU のみ
docker compose -f docker-compose.yml -f docker-compose.cpu.yml up -d
```

ブラウザで http://localhost:8080 を開いてください。

### CLI での PDF 変換

```bash
# イメージをビルド
docker compose build

# 変換実行 (GPU使用)
docker run --rm --gpus all \
  -v $(pwd)/data/input:/data/input:ro \
  -v $(pwd)/data/output:/data/output:rw \
  superbook-rust-core:latest \
  convert /data/input/book.pdf -o /data/output/ --advanced --ocr
```

### Podman の場合

```bash
# NVIDIA GPU
podman compose up -d

# AMD GPU (ROCm)
podman compose -f docker-compose.yml -f docker-compose.rocm.yml up -d

# CPU のみ
podman compose -f docker-compose.yml -f docker-compose.cpu.yml up -d

# CLI 変換
podman run --rm --device nvidia.com/gpu=all \
  -v $(pwd)/data/input:/data/input:ro \
  -v $(pwd)/data/output:/data/output:rw \
  superbook-rust-core:latest \
  convert /data/input/book.pdf -o /data/output/ --advanced --ocr
```

---

## CPU 専用 Dockerfile

GPU なしの環境向けに `Dockerfile.cpu` を提供しています。

```
docker/backend/Dockerfile.cpu
```

特徴:
- ベースイメージ: `ubuntu:22.04` (CUDA 不要)
- Rust Core の API/WS サーバーのみを実行
- 環境変数 `SUPERBOOK_NO_GPU=1` が設定済み

### 手動ビルド

```bash
docker build -f docker/backend/Dockerfile.cpu -t superbook-rust-core-cpu:latest superbook-pdf/
```

---

## Dockerfile の構成

```
superbook-pdf/
└── Dockerfile             # NVIDIA (デフォルト)

docker/
└── backend/
  ├── Dockerfile.cpu     # CPU のみ
  └── Dockerfile.rocm    # AMD GPU (ROCm)
```

### マルチステージビルド

各 Dockerfile は以下の構成です:

1. **Rust ビルドステージ** - `cargo build --release --features web`
2. **ランタイムステージ** - 最終イメージ (poppler-utils + バイナリ)

---

## ボリュームマウント

| ホスト | コンテナ | 用途 |
|--------|---------|------|
| `./data/input` | `/data/input` | 入力 PDF (読み取り専用) |
| `./data/output` | `/data/output` | 出力ファイル |
| `./data/work` | `/data/work` | 中間ファイル |

---

## 環境変数

コンテナ内で使える環境変数:

| 変数 | デフォルト | 説明 |
|------|-----------|------|
| `SUPERBOOK_PORT` | 8080 | Rust API/WS のポート |
| `SUPERBOOK_BIND` | 0.0.0.0 | バインドアドレス |
| `SUPERBOOK_NO_GPU` | (未設定) | `1` で GPU 無効化 |
| `REALESRGAN_API_URL` | `http://realesrgan-api:8000` | 超解像 API URL |
| `YOMITOKU_API_URL` | `http://yomitoku-api:8000` | OCR API URL |

---

## Web UI のアップロード上限

Web UI 経由の PDF アップロードは、以下 2 つの上限を両方満たす必要があります。

1. フロント (Nginx) の `client_max_body_size`
2. バックエンド (Axum) の request body / multipart 上限

本リポジトリのデフォルトは **500MB** です。

- Nginx: `web_ui/default.conf.template` の `client_max_body_size 500m;`
- バックエンド: `superbook-pdf serve --upload-limit 500` (MB)

`--upload-limit` を変更する場合は、Nginx 側の `client_max_body_size` も同等以上に合わせてください。

---

## トラブルシューティング

### 複数ページ PDF で Upload failed になる

Web UI で複数ページ PDF が失敗する場合、まず HTTP ステータスを確認してください。

- `413` の場合: Nginx の `client_max_body_size` が小さい
- `400` の場合: バックエンドの multipart 読み取り制限やアップロード上限不一致の可能性

設定変更後は、対象コンテナを再ビルド・再起動してください。

```bash
docker compose build frontend rust-core-stable
docker compose up -d --force-recreate frontend rust-core-stable
```

### GPU が認識されない

```bash
# NVIDIA GPU の確認
nvidia-smi

# Docker で GPU が使えるか確認
docker run --rm --gpus all nvidia/cuda:12.1.1-base-ubuntu22.04 nvidia-smi
```

`nvidia-container-toolkit` がインストールされているか確認してください。

### ビルドが遅い

Rust のコンパイルに時間がかかります。初回ビルドは 10-30 分程度かかることがあります。Docker のビルドキャッシュを活用してください。

### メモリ不足

大きな PDF を処理する場合、Docker のメモリ制限に注意してください:

```bash
# メモリ制限を 8GB に設定
docker run --rm --gpus all --memory=8g \
  -v $(pwd)/data/input:/data/input:ro \
  -v $(pwd)/data/output:/data/output:rw \
  superbook-rust-core:latest \
  convert /data/input/large_book.pdf -o /data/output/ --chunk-size 5
```
