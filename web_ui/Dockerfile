# 1. ベースイメージの指定 (軽量な Alpine Linux 版 Nginx)
FROM docker.io/nginx:1.25-alpine

LABEL maintainer="superbook-pdf"
LABEL description="SuperBook Frontend Web UI (Nginx)"
LABEL component="frontend"

# 2. 環境変数置換用の envsubst をインストール
RUN apk add --no-cache gettext

# 3. デフォルト設定の削除
RUN rm /etc/nginx/conf.d/default.conf

# 4. Nginx 設定ファイルとテンプレートのコピー
COPY nginx.conf /etc/nginx/nginx.conf
COPY default.conf.template /etc/nginx/templates/default.conf.template

# 5. 静的ファイルのコピー (事前に static/ フォルダへ移動済みであること)
COPY static/ /usr/share/nginx/html/

# 6. バックエンド（Rust Core）接続用のデフォルト値
# 注: Docker Compose ネットワークではサービス名が DNS 名として使用される
# docker-compose.yml の service 名: rust-core-stable
ENV BACKEND_HOST=rust-core-stable
ENV BACKEND_PORT=8080

# HTTP ポートの公開
EXPOSE 80

# 7. 起動時に環境変数をテンプレートに適用してから Nginx を開始
CMD ["/bin/sh", "-c", "envsubst '${BACKEND_HOST} ${BACKEND_PORT}' < /etc/nginx/templates/default.conf.template > /etc/nginx/conf.d/default.conf && nginx -g 'daemon off;'"]
