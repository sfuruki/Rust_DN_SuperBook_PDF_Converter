# ===== Stage 1: Build Rust binary =====
# ビルド環境には最新の Rust 公式イメージを使用
FROM docker.io/rust:1.87-bookworm AS builder

# ビルドに必要なシステム依存パッケージのインストール
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# 依存関係のキャッシュを利用するため、先に Cargo ファイルをコピー
COPY Cargo.toml Cargo.lock ./

# ダミーのソースで依存ライブラリのみをプリビルド
RUN mkdir src && echo "fn main() {}" > src/main.rs && \
    mkdir -p src/api_server && touch src/lib.rs
RUN cargo build --release --features web 2>/dev/null || true
RUN rm -rf src

# 実際のソースコードをコピーしてビルド
COPY src ./src
COPY benches ./benches
RUN touch src/main.rs && cargo build --release --features web

# ===== Stage 2: Runtime image =====
# 実行環境は軽量な Debian Bookworm Slim を使用
# AI処理は別コンテナに分離するため、ここでは CUDA や Python は不要
FROM docker.io/debian:bookworm-slim

LABEL maintainer="superbook-pdf"
LABEL description="SuperBook Core Orchestrator (Rust)"
LABEL component="superbook-pdf"

ENV DEBIAN_FRONTEND=noninteractive
ENV TZ=Asia/Tokyo

# nvidia-smi is provided by non-free NVIDIA packages on Debian.
RUN sed -i 's/Components: main/Components: main contrib non-free non-free-firmware/' /etc/apt/sources.list.d/debian.sources

# PDF操作に必要な外部ツールのインストール
RUN apt-get update && apt-get install -y --no-install-recommends \
    # PDF抽出・操作ツール [3-5]
    poppler-utils \
    ghostscript \
    qpdf \
    imagemagick \
    # Optional OCR / page-number detection
    tesseract-ocr \
    # GPU utility command for direct runtime diagnostics
    nvidia-smi \
    # 日本語フォント (PDF生成時のOCRレイヤー埋め込み用) [4-6]
    fonts-noto-cjk \
    fonts-noto-cjk-extra \
    # 実行用ライブラリと診断ツール
    libssl3 \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# 🚀 以下の行を追加して、ImageMagick の PDF 制限を解除します
RUN sed -i 's/rights="none" pattern="PDF"/rights="read|write" pattern="PDF"/' /etc/ImageMagick-6/policy.xml

# アプリケーションディレクトリの設定
WORKDIR /app

# ビルドステージからバイナリのみをコピー
COPY --from=builder /build/target/release/superbook-pdf /usr/local/bin/

# 実行用データディレクトリの作成 (4層構成のボリュームマウント用) [5, 7]
RUN mkdir -p /data/input /data/output /data/work
ENV SUPERBOOK_WORK_DIR=/data/work

# 環境変数のデフォルト設定
ENV RUST_LOG=info
ENV SUPERBOOK_BIND=0.0.0.0
ENV SUPERBOOK_PORT=8080

# API ポートの公開
EXPOSE 8080

# ヘルスチェックの設定 (api/stats エンドポイントを使用) [5, 7]
HEALTHCHECK --interval=30s --timeout=10s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:8080/api/stats || exit 1

# アプリケーションの起動
ENTRYPOINT ["superbook-pdf"]
CMD ["serve", "--bind", "0.0.0.0", "--port", "8080"]
