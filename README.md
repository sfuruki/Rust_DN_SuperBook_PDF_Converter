<p align="center">
  <b>🌐 Language / 言語</b><br>
  <b>日本語</b> |
  <a href="docs/readmeLangs/README.en.md">English</a> |
  <a href="docs/readmeLangs/README.zh-CN.md">简体中文</a> |
  <a href="docs/readmeLangs/README.zh-TW.md">繁體中文</a> |
  <a href="docs/readmeLangs/README.ru.md">Русский</a> |
  <a href="docs/readmeLangs/README.uk.md">Українська</a> |
  <a href="docs/readmeLangs/README.fa.md">فارسی</a> |
  <a href="docs/readmeLangs/README.ar.md">العربية</a>
</p>

# superbook-pdf

[![License: AGPL v3](https://img.shields.io/badge/License-AGPL%20v3-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)
[![Rust](https://img.shields.io/badge/rust-1.82%2B-orange.svg)](https://www.rust-lang.org/)
[![CI](https://github.com/sfuruki/Rust_DN_SuperBook_Reforge/actions/workflows/ci.yml/badge.svg)](https://github.com/sfuruki/Rust_DN_SuperBook_Reforge/actions/workflows/ci.yml)

> **Fork lineage:**
>
> - [dnobori/DN_SuperBook_PDF_Converter](https://github.com/dnobori/DN_SuperBook_PDF_Converter) (Original)
> - [clearclown/Rust_DN_SuperBook_PDF_Converter](https://github.com/clearclown/Rust_DN_SuperBook_PDF_Converter) (Rust fork)
>
> Rust_DN_SuperBook_Reforge は、DN_SuperBook_PDF_Converter と Rust_DN_SuperBook_PDF_Converter の流れを引き継いだ派生プロジェクトです。
>
> 基本的な変換機能は踏襲しつつ、現在の利用環境に合わせて構成や運用面を整理し、機能追加や調整を行いやすい形にしています。
>
> CLI と Web API の両方で扱いやすいことを重視し、拡張しやすさを意識した構成にしています。

**オリジナル著者:** 登 大遊 (Daiyuu Nobori) 様
**Rust リライト:** clearclown 様
**派生・調整:** sfuruki
**ライセンス:** AGPL v3.0

---

## Before / After

![Before and After comparison](doc_img/ba.png)

| | Before (左) | After (右) |
|---|---|---|
| **解像度** | 1242x2048 px | 2363x3508 px |
| **ファイルサイズ** | 981 KB | 1.6 MB |
| **品質** | ぼやけ、低コントラスト | 鮮明、高コントラスト |

RealESRGAN による AI 超解像で、文字のエッジが鮮明になり、読みやすさが大幅に向上します。

---

## 特徴

- **Rust 実装** - C# 版を完全リライト。メモリ効率とパフォーマンスが大幅に改善
- **AI 超解像** - RealESRGAN で画像を 2x 高解像度化
- **日本語 OCR** - YomiToku による高精度文字認識
- **Markdown 変換** - PDF から構造化された Markdown を生成 (図・表の自動検出付き)
- **傾き補正** - 大津二値化 + Hough 変換で自動補正
- **180度回転検出** - 上下逆のページを自動検出・補正
- **影除去** - 製本時の影を自動検出・除去
- **マーカー除去** - 蛍光ペンのハイライトを検出・除去
- **ブレ補正** - ぼやけた画像のシャープ化 (Unsharp Mask / NAFNet / DeblurGAN-v2)
- **カラー補正** - HSV 裏写り抑制、紙色の白化
- **Web UI** - Nginx 配信のブラウザインターフェース

---

## クイックスタート

```bash
# WSL 上でリポジトリ直下に移動
cd Rust_DN_SuperBook_Reforge

# コンテナ起動 (Web UI + API + AI Services)
docker compose up -d

# 入力PDFを配置 (WSL 側)
cp /path/to/book.pdf ./data/input/

# CLI変換 (input指定のみ。出力先は /data/output)
docker compose exec rust-core-stable sh -lc 'superbook-pdf convert /data/input/book.pdf'

# 出力確認
ls ./data/output/
```

---

## コマンド体系

superbook-pdf は 5 つのサブコマンドを提供します:

| コマンド | 説明 |
|---------|------|
| [`convert`](#convert---pdf-高品質化) | PDF を AI 強化して高品質 PDF に変換 |
| [`markdown`](#markdown---pdf-から-markdown-変換) | PDF から構造化された Markdown を生成 |
| [`reprocess`](#reprocess---失敗ページの再処理) | 変換に失敗したページを再処理 |
| `info` | システム環境情報を表示 (GPU、依存ツール等) |
| `cache-info` | 出力 PDF のキャッシュ情報を表示 |

### `convert` - PDF 高品質化

スキャンした PDF を AI 強化して高品質 PDF に変換します。

```bash
# 基本 (傾き補正 + マージントリム + AI超解像)
superbook-pdf convert input.pdf -o output/

# 最高品質 (全機能有効)
superbook-pdf convert input.pdf -o output/ --advanced --ocr

# 影除去 + マーカー除去 + ブレ補正
superbook-pdf convert input.pdf -o output/ --shadow-removal auto --remove-markers --deblur

# テスト (最初の5ページ、実行計画のみ)
superbook-pdf convert input.pdf -o output/ --max-pages 5 --dry-run
```

**主なオプション:**

| オプション | デフォルト | 説明 |
|-----------|-----------|------|
| `-o, --output <DIR>` | `$SUPERBOOK_OUTPUT_DIR` または `./output` | 出力先ディレクトリ |
| `--work-dir <DIR>` | `$SUPERBOOK_WORK_DIR` または output同階層 | 中間ファイル用ディレクトリ |
| `--advanced` | off | 高品質処理を有効化 (内部解像度正規化 + カラー補正 + オフセット調整) |
| `--ocr` | off | 日本語OCR を有効化 |
| `--dpi <N>` | 300 | 出力 DPI |
| `--jpeg-quality <N>` | 90 | PDF 内 JPEG 圧縮品質 (1-100) |
| `-m, --margin-trim <N>` | 0.7 | マージントリム率 (%) |
| `--shadow-removal <MODE>` | auto | 影除去モード (none/auto/left/right/both) |
| `--remove-markers` | off | 蛍光マーカー除去を有効化 |
| `--deblur` | off | ブレ補正を有効化 |
| `--no-upscale` | - | AI超解像をスキップ |
| `--no-deskew` | - | 傾き補正をスキップ |
| `--no-gpu` | - | GPU処理を無効化 |
| `--dry-run` | - | 実行計画を表示 (実処理なし) |
| `--max-pages <N>` | - | 処理ページ数を制限 |
| `-v, -vv, -vvv` | - | ログ詳細度 |

全オプションは `superbook-pdf convert --help` で確認できます。

### `markdown` - PDF から Markdown 変換

PDF を OCR し、構造化された Markdown に変換します。図の自動検出・抽出、表の検出、読み順序の自動判定に対応しています。

```bash
# 基本変換
superbook-pdf markdown input.pdf -o output/

# 縦書き指定 + AI超解像
superbook-pdf markdown input.pdf -o output/ --text-direction vertical --upscale

# 品質検証付き
superbook-pdf markdown input.pdf -o output/ --validate --api-provider claude

# 中断した処理を再開
superbook-pdf markdown input.pdf -o output/ --resume
```

**主なオプション:**

| オプション | デフォルト | 説明 |
|-----------|-----------|------|
| `-o, --output <DIR>` | `./markdown_output` | 出力先ディレクトリ |
| `--text-direction` | auto | テキスト方向 (auto/horizontal/vertical) |
| `--upscale` | off | OCR前にAI超解像を適用 |
| `--dpi <N>` | 300 | 出力DPI |
| `--figure-sensitivity <N>` | - | 図検出の感度 (0.0-1.0) |
| `--no-extract-images` | - | 画像抽出を無効化 |
| `--no-detect-tables` | - | 表検出を無効化 |
| `--include-page-numbers` | off | ページ番号を出力に含める |
| `--validate` | off | 出力Markdownの品質検証 |
| `--resume` | - | 中断した処理を再開 |

全オプションは `superbook-pdf markdown --help` で確認できます。

### `reprocess` - 失敗ページの再処理

変換中にエラーが発生したページだけを再処理します。

```bash
# 状態ファイルから自動検出して再処理
superbook-pdf reprocess output/.superbook-state.json

# 特定ページのみ再処理
superbook-pdf reprocess output/.superbook-state.json -p 5,12,30

# 状態確認のみ
superbook-pdf reprocess output/.superbook-state.json --status
```

---

## 処理パイプライン

`convert` コマンドの処理フロー:

```
入力PDF
  │
  ├─ Step 1: PDF画像抽出 (pdftoppm, 指定DPI)
  ├─ Step 2: マージントリム (デフォルト 0.7%)
  ├─ Step 3: 影除去 (--shadow-removal)
  ├─ Step 4: AI超解像 (RealESRGAN 2x)
  ├─ Step 5: ブレ補正 (--deblur)
  ├─ Step 6: 180度回転検出・補正
  ├─ Step 7: 傾き補正 (大津二値化 + Hough変換)
  ├─ Step 8: カラー補正 (HSV裏写り抑制)
  ├─ Step 9: マーカー除去 (--remove-markers)
  ├─ Step 10: グループクロップ (均一マージン)
  ├─ Step 11: PDF生成 (JPEG DCT圧縮)
  └─ Step 12: OCR (YomiToku, --ocr)
  │
  出力PDF
```

空白ページは自動検出 (閾値 2%) され、処理をスキップします。

---

## インストール

### 必要なもの

| 項目 | 要件 |
|------|------|
| OS | Windows 11 + WSL2 (Ubuntu 推奨) |
| Docker | Docker Desktop + WSL integration |
| GPU (任意) | NVIDIA GPU + Docker GPU runtime |

このREADMEでは、WSL + Docker構成のみを対象に説明します。

### 1. リポジトリの配置

```bash
git clone https://github.com/sfuruki/Rust_DN_SuperBook_Reforge.git
cd Rust_DN_SuperBook_Reforge
```

### 2. コンテナ起動

```bash
# NVIDIA GPU
docker compose up -d

# CPUのみ
docker compose -f docker-compose.yml -f docker-compose.cpu.yml up -d
```

ブラウザで http://localhost:8080 を開けば使えます。

### 3. CLI 実行例 (WSL + Docker)

```bash
# input だけ指定 (出力: /data/output, 中間: /data/work)
docker compose exec rust-core-stable sh -lc 'superbook-pdf convert /data/input/book.pdf'

# 明示指定したい場合
docker compose exec rust-core-stable sh -lc 'superbook-pdf convert /data/input/book.pdf -o /data/output --work-dir /data/work'
```

---

## Web UI

![Web UI](doc_img/webUI.png)

ブラウザベースのインターフェースで、ファイルをドラッグ&ドロップするだけで変換が始まります。WebSocket によるリアルタイム進捗表示に対応しています。

```bash
# 推奨: フロントエンド(Nginx) + バックエンド(Rust API/WS)
docker compose up -d

# 直接起動する場合は Rust 側 API/WS サーバーのみ
superbook-pdf serve --port 8080 --bind 0.0.0.0
```

---

## 詳細ドキュメント

| ドキュメント | 内容 |
|-------------|------|
| [docs/pipeline.md](docs/pipeline.md) | 処理パイプラインの詳細設計 |
| [docs/commands.md](docs/commands.md) | 全コマンド・全オプションのリファレンス |
| [docs/configuration.md](docs/configuration.md) | 設定ファイル (TOML) によるカスタマイズ |
| [docs/docker.md](docs/docker.md) | Docker/Podman 環境の詳細ガイド |
| [docs/development.md](docs/development.md) | 開発者向けガイド (ビルド、テスト、アーキテクチャ) |

---

## トラブルシューティング

| 問題 | 解決策 |
|------|--------|
| `pdftoppm: command not found` | `sudo apt install poppler-utils` |
| RealESRGAN が動かない | `docker compose ps` と `superbook-pdf info` で AI Services の状態を確認 |
| GPU が使用されない | `docker compose ps` と `nvidia-smi` でコンテナ/ドライバ状態を確認し、必要なら CPU override (`-f docker-compose.cpu.yml`) を使用 |
| メモリ不足 | `--max-pages 10` か `--chunk-size 5` で分割処理 |
| 傾き補正で画像が崩れる | `--no-deskew` で無効化 |
| マージンで文字が切れる | `--margin-safety 1.0` で安全バッファを増加 |

---

## ライセンス

AGPL v3.0 - [LICENSE](LICENSE)

---

## 謝辞

- **登 大遊 (Daiyuu Nobori) 様** - オリジナル実装
- **clearclown 様** - Rust リライト版の実装
- **[RealESRGAN](https://github.com/xinntao/Real-ESRGAN)** - AI超解像
- **[YomiToku](https://github.com/kotaro-kinoshita/yomitoku)** - 日本語OCR

---

## 開発について

本プロジェクトの開発では、GitHub Copilot を活用して設計・実装・検証を進めています。
