<p align="center">
  <b>🌐 语言</b><br>
  <a href="../../README.md">日本語</a> |
  <a href="README.en.md">English</a> |
  <b>简体中文</b> |
  <a href="README.zh-TW.md">繁體中文</a> |
  <a href="README.ru.md">Русский</a> |
  <a href="README.uk.md">Українська</a> |
  <a href="README.fa.md">فارسی</a> |
  <a href="README.ar.md">العربية</a>
</p>

# superbook-pdf

[![License: AGPL v3](https://img.shields.io/badge/License-AGPL%20v3-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)
[![Rust](https://img.shields.io/badge/rust-1.82%2B-orange.svg)](https://www.rust-lang.org/)
[![CI](https://github.com/sfuruki/Rust_DN_SuperBook_Reforge/actions/workflows/ci.yml/badge.svg)](https://github.com/sfuruki/Rust_DN_SuperBook_Reforge/actions/workflows/ci.yml)

> **Fork 传承关系：**
>
> - [dnobori/DN_SuperBook_PDF_Converter](https://github.com/dnobori/DN_SuperBook_PDF_Converter)（原始版）
> - [clearclown/Rust_DN_SuperBook_PDF_Converter](https://github.com/clearclown/Rust_DN_SuperBook_PDF_Converter)（Rust 分支）
>
> Rust_DN_SuperBook_Reforge 是沿着 DN_SuperBook_PDF_Converter 与 Rust_DN_SuperBook_PDF_Converter 脉络继续发展的派生项目。
>
> 它保留了原有的核心转换功能，同时针对当前运行环境重新整理了结构与运维方式，以便后续扩展和维护。
>
> 这个派生版主要着重于 AI 执行环境分离与 HTTP 微服务化、页面级部分并行执行，以及 Web UI / WebSocket 进度显示的细化。

**原作者:** 登 大遊 (Daiyuu Nobori)
**Rust 重写:** clearclown
**派生与调整:** sfuruki
**许可证:** AGPL v3.0

---

## 处理前 / 处理后

![处理前后对比](../doc_img/ba.png)

| | 处理前 (左) | 处理后 (右) |
|---|---|---|
| **分辨率** | 1242x2048 px | 2363x3508 px |
| **文件大小** | 981 KB | 1.6 MB |
| **质量** | 模糊、低对比度 | 清晰、高对比度 |

通过 RealESRGAN AI 超分辨率技术，文字边缘变得锐利，可读性大幅提升。

---

## 功能特性

- **Rust 实现** - 从 C# 完全重写，内存效率和性能大幅提升
- **AI 运行环境分离** - 将 RealESRGAN / YomiToku 从 Rust Core 中拆分出来，通过 Docker/Podman 以 HTTP 微服务方式运行
- **AI 超分辨率** - 使用 RealESRGAN 进行 2 倍图像放大
- **日文 OCR** - 使用 YomiToku 进行高精度文字识别
- **Markdown 转换** - 从 PDF 生成结构化 Markdown（自动检测图表）
- **部分并行执行** - 支持页面级并行处理，可通过 `--threads` 与 `--chunk-size` 控制负载和内存占用
- **进度显示细化** - 在既有 Web UI / WebSocket 基础上，细化按处理阶段展示的进度与日志
- **倾斜校正** - 通过大津二值化 + 霍夫变换自动校正
- **180度旋转检测** - 自动检测和校正上下颠倒的页面
- **阴影去除** - 自动检测和去除装订阴影
- **标记去除** - 检测和去除荧光笔标记
- **去模糊** - 锐化模糊图像 (Unsharp Mask / NAFNet / DeblurGAN-v2)
- **色彩校正** - HSV 透印抑制、纸张白化
- **Web UI** - 通过浏览器直观操作

---

## 快速开始

```bash
# 从源码构建
git clone https://github.com/sfuruki/Rust_DN_SuperBook_Reforge.git
cd Rust_DN_SuperBook_Reforge/superbook-pdf
cargo build --release --features web

# 基本转换
superbook-pdf convert input.pdf -o output/

# 高质量转换（AI 超分辨率 + 色彩校正 + 偏移对齐）
superbook-pdf convert input.pdf -o output/ --advanced --ocr

# Markdown 转换
superbook-pdf markdown input.pdf -o markdown_output/

# 启动 Web UI
docker compose up -d
```

---

## 命令体系

superbook-pdf 提供 5 个子命令:

| 命令 | 说明 |
|------|------|
| `convert` | 使用 AI 增强 PDF 生成高质量 PDF |
| `markdown` | 从 PDF 生成结构化 Markdown |
| `reprocess` | 重新处理转换失败的页面 |
| `info` | 显示系统环境信息（GPU、依赖工具等） |
| `cache-info` | 显示输出 PDF 的缓存信息 |

### `convert` - PDF 高质量增强

```bash
# 基本（倾斜校正 + 边距修剪 + AI 超分辨率）
superbook-pdf convert input.pdf -o output/

# 最高质量（所有功能启用）
superbook-pdf convert input.pdf -o output/ --advanced --ocr

# 阴影去除 + 标记去除 + 去模糊
superbook-pdf convert input.pdf -o output/ --shadow-removal auto --remove-markers --deblur

# 测试（前 5 页，仅计划）
superbook-pdf convert input.pdf -o output/ --max-pages 5 --dry-run
```

**主要选项：**

| 选项 | 默认值 | 说明 |
|------|--------|------|
| `-o, --output <DIR>` | `./output` | 输出目录 |
| `--advanced` | 关 | 高质量处理（内部分辨率 + 色彩校正 + 偏移对齐） |
| `--ocr` | 关 | 日文 OCR |
| `--dpi <N>` | 300 | 输出 DPI |
| `--jpeg-quality <N>` | 90 | PDF 中 JPEG 压缩质量 (1-100) |
| `-m, --margin-trim <N>` | 0.7 | 边距修剪百分比 (%) |
| `--shadow-removal <MODE>` | auto | 阴影去除模式 (none/auto/left/right/both) |
| `--remove-markers` | 关 | 荧光笔标记去除 |
| `--deblur` | 关 | 去模糊处理 |
| `--no-upscale` | — | 跳过 AI 超分辨率 |
| `--no-deskew` | — | 跳过倾斜校正 |
| `--no-gpu` | — | 禁用 GPU |
| `--dry-run` | — | 仅显示执行计划（不处理） |
| `--max-pages <N>` | — | 限制处理页数 |

### `markdown` - PDF 转 Markdown

```bash
# 基本转换
superbook-pdf markdown input.pdf -o output/

# 指定竖排文本 + AI 超分辨率
superbook-pdf markdown input.pdf -o output/ --text-direction vertical --upscale

# 恢复中断的处理
superbook-pdf markdown input.pdf -o output/ --resume
```

**主要选项：**

| 选项 | 默认值 | 说明 |
|------|--------|------|
| `-o, --output <DIR>` | `./markdown_output` | 输出目录 |
| `--text-direction` | auto | 文字方向 (auto/horizontal/vertical) |
| `--upscale` | 关 | OCR 前先执行 AI 超分辨率 |
| `--dpi <N>` | 300 | 输出 DPI |
| `--figure-sensitivity <N>` | — | 图片检测灵敏度 (0.0-1.0) |
| `--no-extract-images` | — | 禁用图片提取 |
| `--no-detect-tables` | — | 禁用表格检测 |
| `--validate` | 关 | 输出 Markdown 质量验证 |
| `--resume` | — | 恢复中断的处理 |

### `reprocess` - 重新处理失败页面

```bash
# 自动检测并重新处理
superbook-pdf reprocess output/.superbook-state.json

# 仅重新处理指定页面
superbook-pdf reprocess output/.superbook-state.json -p 5,12,30

# 仅查看状态
superbook-pdf reprocess output/.superbook-state.json --status
```

---

## 处理流水线

```
输入 PDF
  |
  +- Step 1:  PDF 图像提取 (pdftoppm)
  +- Step 2:  边距修剪 (默认 0.7%)
  +- Step 3:  阴影去除
  +- Step 4:  AI 超分辨率 (RealESRGAN 2x)
  +- Step 5:  去模糊
  +- Step 6:  180度旋转检测/校正
  +- Step 7:  倾斜校正 (大津二值化 + 霍夫变换)
  +- Step 8:  色彩校正 (HSV 透印抑制)
  +- Step 9:  标记去除
  +- Step 10: 分组裁剪 (统一边距)
  +- Step 11: PDF 生成 (JPEG DCT 压缩)
  +- Step 12: OCR (YomiToku)
  |
  输出 PDF
```

空白页自动检测（阈值 2%）并跳过所有处理。

---

## 安装

### 系统要求

| 项目 | 要求 |
|------|------|
| 操作系统 | Linux / macOS / Windows |
| Rust | 1.82+（从源码构建） |
| Poppler | `pdftoppm` 命令 |

AI 功能需要 Python 3.10+ 及 NVIDIA GPU（CUDA 11.8+）。

### 1. 系统依赖

```bash
# Ubuntu/Debian
sudo apt update && sudo apt install -y poppler-utils python3 python3-venv

# Fedora
sudo dnf install -y poppler-utils python3

# macOS (Homebrew)
brew install poppler python

# Windows (Chocolatey)
choco install poppler python
```

### 2. 安装 superbook-pdf

```bash
git clone https://github.com/clearclown/Rust_DN_SuperBook_PDF_Converter.git
cd Rust_DN_SuperBook_PDF_Converter/superbook-pdf
cargo build --release --features web
```

### 3. 通过 Docker/Podman 运行（推荐）

```bash
# NVIDIA GPU
docker compose up -d

# AMD GPU (ROCm)
docker compose -f docker-compose.yml -f docker-compose.rocm.yml up -d

# 仅 CPU
docker compose -f docker-compose.yml -f docker-compose.cpu.yml up -d
```

在浏览器中打开 http://localhost:8080。

---

## Web UI

![Web UI](../doc_img/webUI.png)

浏览器界面，只需拖放文件即可开始转换。支持 WebSocket 实时进度显示。

```bash
# 推荐：前端 (Nginx) + 后端 (Rust API/WS)
docker compose up -d

# 直接模式：仅启动后端 API/WS 服务器
superbook-pdf serve --port 8080 --bind 0.0.0.0
```

---

## 文档

| 文档 | 内容 |
|------|------|
| [docs/pipeline.md](../pipeline.md) | 处理流程详细设计 |
| [docs/commands.md](../commands.md) | 完整命令及选项参考 |
| [docs/configuration.md](../configuration.md) | 配置文件自定义 (TOML) |
| [docs/docker.md](../docker.md) | Docker/Podman 环境详细指南 |
| [docs/development.md](../development.md) | 开发者指南（构建、测试、架构） |

---

## 问题排除

| 问题 | 解决方法 |
|------|----------|
| `pdftoppm: command not found` | `sudo apt install poppler-utils` |
| RealESRGAN 无法工作 | 用 `docker compose ps` 和 `superbook-pdf info` 检查 AI 服务 |
| GPU 未被使用 | 检查 `docker compose ps` 和 `nvidia-smi`；必要时使用 CPU 模式 (`-f docker-compose.cpu.yml`) |
| 内存不足 | 使用 `--max-pages 10` 或 `--chunk-size 5` |
| Deskew 导致图像变形 | 用 `--no-deskew` 禁用 |
| 边距裁剪到文字 | 增加安全缓冲：`--margin-safety 1.0` |

---

## 许可证

AGPL v3.0 — [LICENSE](../../LICENSE)

---

## 致谢

- **Daiyuu Nobori** — 原始实现
- **[RealESRGAN](https://github.com/xinntao/Real-ESRGAN)** — AI 超分辨率
- **[YomiToku](https://github.com/kotaro-kinoshita/yomitoku)** — 日文 OCR

---

## 安装

### 系统要求

| 项目 | 要求 |
|------|------|
| 操作系统 | Linux / macOS / Windows |
| Rust | 1.82 以上（源码构建时） |
| Poppler | `pdftoppm` 命令 |

使用 AI 功能需要 Python 3.10+ 和 NVIDIA GPU (CUDA 11.8+)。

### Docker/Podman 运行（推荐）

```bash
# NVIDIA GPU
docker compose up -d

# AMD GPU (ROCm)
docker compose -f docker-compose.yml -f docker-compose.rocm.yml up -d

# 仅 CPU
docker compose -f docker-compose.yml -f docker-compose.cpu.yml up -d
```

在浏览器中打开 http://localhost:8080。

---

## 详细文档

| 文档 | 内容 |
|------|------|
| [docs/pipeline.md](../pipeline.md) | 处理流水线详细设计 |
| [docs/commands.md](../commands.md) | 全部命令和选项参考 |
| [docs/configuration.md](../configuration.md) | 配置文件 (TOML) 自定义 |
| [docs/docker.md](../docker.md) | Docker/Podman 环境详细指南 |
| [docs/development.md](../development.md) | 开发者指南 |

---

## 许可证

AGPL v3.0 - [LICENSE](../../LICENSE)

## 致谢

- **登 大遊 (Daiyuu Nobori)** - 原始实现
- **[RealESRGAN](https://github.com/xinntao/Real-ESRGAN)** - AI 超分辨率
- **[YomiToku](https://github.com/kotaro-kinoshita/yomitoku)** - 日文 OCR
