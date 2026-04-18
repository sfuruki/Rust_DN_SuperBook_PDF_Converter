# 開発者ガイド

superbook-pdf の開発に参加するためのガイドです。

---

## 環境構築

### 必要なツール

| ツール | バージョン | 用途 |
|--------|----------|------|
| Rust | 1.82+ | コンパイラ |
| Poppler | - | `pdftoppm` (テストで使用) |
| Python | 3.10+ | AI ブリッジ (任意) |

### ビルド

```bash
cd superbook-pdf

# デバッグビルド
cargo build --features web

# リリースビルド
cargo build --release --features web

# Web 機能なし (CLI のみ)
cargo build --release
```

### テスト

```bash
# 全テスト
cargo test --features web

# 特定モジュール
cargo test deskew
cargo test page_number
cargo test markdown_gen

# 単一テスト
cargo test test_otsu_threshold
```

### Lint

```bash
# フォーマット
cargo fmt

# Clippy
cargo clippy --features web -- -D warnings
```

---

## プロジェクト構造

```
superbook-pdf/
├── Cargo.toml
├── src/
│   ├── main.rs               # CLI エントリポイント
│   ├── lib.rs                 # ライブラリエクスポート
│   ├── cli.rs                 # CLI 引数定義 (clap)
│   ├── config.rs              # 設定管理・マージロジック
│   ├── pipeline.rs            # メイン処理パイプライン
│   ├── markdown_pipeline.rs   # Markdown 変換パイプライン
│   ├── markdown_gen.rs        # Markdown 生成・後処理
│   ├── pdf_reader.rs          # PDF 画像抽出
│   ├── pdf_writer.rs          # PDF 生成 (JPEG DCT)
│   ├── deskew/
│   │   ├── mod.rs
│   │   └── algorithm.rs       # 大津二値化 + Hough変換 + 回転検出
│   ├── margin/
│   │   ├── mod.rs
│   │   ├── detect.rs          # マージン検出
│   │   ├── content_aware.rs   # コンテンツ認識マージン
│   │   ├── group.rs           # グループクロップ
│   │   └── shadow.rs          # 影除去
│   ├── cleanup/
│   │   ├── deblur.rs          # ブレ補正
│   │   └── marker_removal.rs  # マーカー除去
│   ├── page_number/
│   │   ├── mod.rs
│   │   ├── detect.rs          # 4段階フォールバックマッチング
│   │   ├── offset.rs          # グループベースオフセット調整
│   │   └── types.rs           # 型定義
│   ├── markdown/
│   │   ├── mod.rs
│   │   ├── converter.rs       # Markdown 変換エンジン
│   │   ├── renderer.rs        # Markdown レンダラー
│   │   ├── element_detect.rs  # 要素検出 (見出し、図、表)
│   │   ├── reading_order.rs   # 読み順序判定
│   │   ├── api_validate.rs    # 外部 API 検証
│   │   └── types.rs           # 型定義
│   ├── color_stats.rs         # HSV カラー補正
│   ├── figure_detect.rs       # 図検出・分類
│   ├── realesrgan.rs          # AI 超解像ブリッジ
│   ├── yomitoku.rs            # OCR ブリッジ
│   ├── util.rs                # ユーティリティ
│   └── web/                   # Web API (feature: web)
│       ├── mod.rs
│       ├── server.rs          # Axum サーバー
│       ├── routes.rs          # REST エンドポイント
│       ├── websocket.rs       # WebSocket ハンドラー
│       ├── job.rs             # ジョブキュー
│       ├── worker.rs          # バックグラウンドワーカー
│       └── ...                # API/WS 補助モジュール
├── tests/                     # 統合テスト
├── specs/                     # TDD 仕様
└── Dockerfile                 # NVIDIA (デフォルト)

../docker/
└── backend/
    ├── Dockerfile.cpu         # CPU
    └── Dockerfile.rocm        # AMD GPU
```

---

## アーキテクチャ

### パイプラインの設計

`pipeline.rs` がメインの処理オーケストレーション。各ステップは `step_*` メソッドとして実装:

```rust
impl Pipeline {
    fn step_margin_trim(&self, ...) -> Result<()>
    fn step_shadow_removal(&self, ...) -> Result<()>
    fn step_upscale(&self, ...) -> Result<()>
    fn step_deblur(&self, ...) -> Result<()>
    fn step_rotation_detect(&self, ...) -> Result<()>
    fn step_deskew(&self, ...) -> Result<()>
    fn step_color_correction(&self, ...) -> Result<()>
    fn step_marker_removal(&self, ...) -> Result<()>
    fn step_group_crop(&self, ...) -> Result<()>
    // ...
}
```

各ステップは:
- 空白ページをスキップ
- 失敗時は元画像をコピー (処理を継続)
- `rayon` で並列処理

### 設定管理

```
CLI 引数 (cli.rs)
    ↓
CliOverrides (config.rs)
    ↓ merge
PipelineConfig (pipeline.rs)
    ↓
Pipeline 実行
```

`PipelineConfig` は:
- serde で TOML シリアライズ/デシリアライズ
- `Default` trait でデフォルト値を定義
- CLI 引数でオーバーライド

### AI サービス連携

AI 機能 (RealESRGAN, YomiToku) は HTTP マイクロサービスとして分離されています:

```
Rust Core → HTTP API → AI Services (RealESRGAN / YomiToku)
```

- Rust Core は HTTP で `/version`, `/upscale`, `/ocr` を呼び出します
- Python / Torch のバージョン差異は各サービスコンテナで独立管理します
- `superbook-pdf info` でサービス単位の状態確認ができます

### Feature Flags

| Feature | 説明 |
|---------|------|
| `web` | Web API/WebSocket (Axum) を有効化 |

```bash
# Web 機能付き
cargo build --features web

# CLI のみ (軽量)
cargo build
```

---

## TDD ワークフロー

```
1. specs/*.spec.md に仕様を記述
2. tests/ にテストを作成 (Red)
3. src/ に実装 (Green)
4. リファクタリング
5. cargo test --features web で全テスト通過を確認
```

---

## CI/CD

GitHub Actions で以下を自動実行:

1. `cargo fmt -- --check` - フォーマットチェック
2. `cargo clippy --features web -- -D warnings` - Lint
3. `cargo test --features web` - テスト

設定: `.github/workflows/ci.yml`

トリガー: `main` ブランチへの push と PR

---

## コントリビューション

1. Issue を確認または作成
2. `main` からフィーチャーブランチを作成
3. 実装 + テスト
4. `cargo fmt && cargo clippy --features web -- -D warnings && cargo test --features web`
5. PR を作成
