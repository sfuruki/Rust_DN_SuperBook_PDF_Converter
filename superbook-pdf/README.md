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

この派生版では、元ソースの変換機能を引き継ぎつつ、特に AI 実行環境の分離と HTTP マイクロサービス化、ページ単位の一部並列実行、Web UI / WebSocket の進捗表示の詳細化に手を入れています。

**オリジナル著者:** 登 大遊 (Daiyuu Nobori) 様
**Rust リライト:** clearclown 様
**派生・調整:** sfuruki
**ライセンス:** AGPL v3.0

---

## Before / After

![Before and After comparison](../docs/doc_img/ba.png)

| | Before (左) | After (右) |
|---|---|---|
| **解像度** | 1242x2048 px | 2363x3508 px |
| **ファイルサイズ** | 981 KB | 1.6 MB |
| **品質** | ぼやけ、低コントラスト | 鮮明、高コントラスト |

RealESRGAN による AI 超解像で、文字のエッジが鮮明になり、読みやすさが大幅に向上します。

---

## 特徴

- **Rust 実装** - C# 版を完全リライト。メモリ効率とパフォーマンスが大幅に改善
- **AI 実行環境の分離** - RealESRGAN / YomiToku を Rust Core から切り離し、Docker/Podman で扱いやすい HTTP マイクロサービスとして運用
- **AI 超解像** - RealESRGAN で画像を 2x 高解像度化
- **日本語 OCR** - YomiToku による高精度文字認識
- **Markdown 変換** - PDF から構造化された Markdown を生成 (図・表の自動検出付き)
- **一部並列実行** - ページ単位の並列処理に対応し、`--threads` や `--chunk-size` で負荷とメモリ使用量を調整可能
- **進捗表示の詳細化** - 既存の Web UI / WebSocket 基盤の上で、処理段階ごとの進捗やログを把握しやすく改善
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
# ビルド
cargo build --release --features web

# 基本変換
superbook-pdf convert input.pdf -o output/

# 高品質変換
superbook-pdf convert input.pdf -o output/ --advanced --ocr

# Markdown 変換
superbook-pdf markdown input.pdf -o markdown_output/

# Web UI 起動 (推奨: Nginx + API)
docker compose up -d
```

---

## コマンド体系

| コマンド | 説明 |
|---------|------|
| `convert` | PDF を AI 強化して高品質 PDF に変換 |
| `markdown` | PDF から構造化された Markdown を生成 |
| `reprocess` | 変換に失敗したページを再処理 |
| `info` | システム環境情報を表示 |
| `cache-info` | 出力 PDF のキャッシュ情報を表示 |

詳細は [プロジェクトルートの README](../README.md) と [docs/](../docs/) を参照してください。

---

## 処理パイプライン

```
入力PDF → 画像抽出 → マージントリム → 影除去 → AI超解像 → ブレ補正
  → 回転検出 → 傾き補正 → カラー補正 → マーカー除去
  → グループクロップ → PDF生成 → OCR → 出力PDF
```

詳細: [docs/pipeline.md](../docs/pipeline.md)

---

## Docker

```bash
# NVIDIA GPU
docker compose up -d

# CPU のみ
docker compose -f docker-compose.yml -f docker-compose.cpu.yml up -d
```

詳細: [docs/docker.md](../docs/docker.md)

---

## 開発

```bash
cargo test --features web
cargo clippy --features web -- -D warnings
cargo fmt
```

詳細: [docs/development.md](../docs/development.md)

---

## ライセンス

AGPL v3.0 - [LICENSE](../LICENSE)
