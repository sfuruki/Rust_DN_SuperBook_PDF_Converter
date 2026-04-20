<div dir="rtl">

<p align="center">
  <b>🌐 زبان</b><br>
  <a href="../../README.md">日本語</a> |
  <a href="README.en.md">English</a> |
  <a href="README.zh-CN.md">简体中文</a> |
  <a href="README.zh-TW.md">繁體中文</a> |
  <a href="README.ru.md">Русский</a> |
  <a href="README.uk.md">Українська</a> |
  <b>فارسی</b> |
  <a href="README.ar.md">العربية</a>
</p>

# superbook-pdf

[![License: AGPL v3](https://img.shields.io/badge/License-AGPL%20v3-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)
[![Rust](https://img.shields.io/badge/rust-1.82%2B-orange.svg)](https://www.rust-lang.org/)
[![CI](https://github.com/sfuruki/Rust_DN_SuperBook_Reforge/actions/workflows/ci.yml/badge.svg)](https://github.com/sfuruki/Rust_DN_SuperBook_Reforge/actions/workflows/ci.yml)

> **تبار فورک:**
>
> - [dnobori/DN_SuperBook_PDF_Converter](https://github.com/dnobori/DN_SuperBook_PDF_Converter) (نسخه اصلی)
> - [clearclown/Rust_DN_SuperBook_PDF_Converter](https://github.com/clearclown/Rust_DN_SuperBook_PDF_Converter) (فورک Rust)
>
> Rust_DN_SuperBook_Reforge یک پروژه مشتق‌شده است که مسیر DN_SuperBook_PDF_Converter و Rust_DN_SuperBook_PDF_Converter را ادامه می‌دهد.
>
> این پروژه قابلیت‌های اصلی تبدیل را حفظ کرده، اما ساختار اجرا و بهره‌برداری را برای محیط‌های امروزی بازتنظیم می‌کند تا توسعه و نگهداری آسان‌تر شود.
>
> در این نسخه مشتق‌شده، تمرکز اصلی روی جداسازی محیط اجرای AI و ریزسرویس‌سازی HTTP، اجرای موازی بخشی از پردازش در سطح صفحه، و جزئی‌تر کردن نمایش پیشرفت در Web UI / WebSocket است.

**نویسنده اصلی:** دایو نوبوری (登 大遊)
**بازنویسی Rust:** clearclown
**نسخه مشتق‌شده / تنظیمات:** sfuruki
**مجوز:** AGPL v3.0

---

## قبل / بعد

![مقایسه قبل و بعد](../doc_img/ba.png)

| | قبل (چپ) | بعد (راست) |
|---|---|---|
| **وضوح** | 1242x2048 پیکسل | 2363x3508 پیکسل |
| **حجم فایل** | 981 کیلوبایت | 1.6 مگابایت |
| **کیفیت** | تار، کنتراست پایین | واضح، کنتراست بالا |

فوق‌وضوح هوش مصنوعی با RealESRGAN لبه‌های متن را تیز کرده و خوانایی را به طرز چشمگیری بهبود می‌بخشد.

---

## ویژگی‌ها

- **پیاده‌سازی با Rust** - بازنویسی کامل از C#. بهبود چشمگیر کارایی حافظه و عملکرد
- **جداسازی محیط اجرای AI** - RealESRGAN / YomiToku از Rust Core جدا شده‌اند و از طریق Docker/Podman به‌صورت ریزسرویس HTTP اجرا می‌شوند
- **فوق‌وضوح AI** - بزرگ‌نمایی ۲ برابری با RealESRGAN
- **OCR ژاپنی** - تشخیص متن با دقت بالا توسط YomiToku
- **تبدیل به Markdown** - تولید Markdown ساختارمند از PDF (با تشخیص خودکار تصاویر و جداول)
- **اجرای موازی بخشی از پردازش** - پردازش موازی در سطح صفحه با کنترل بار و حافظه از طریق `--threads` و `--chunk-size`
- **جزئیات بیشتر در نمایش پیشرفت** - بهبود نمایش مرحله‌به‌مرحله پیشرفت و لاگ‌ها بر بستر موجود Web UI / WebSocket
- **تصحیح انحراف** - تصحیح خودکار از طریق دوتایی‌سازی اوتسو + تبدیل هاف
- **تشخیص چرخش ۱۸۰ درجه** - تشخیص و تصحیح خودکار صفحات وارونه
- **حذف سایه** - تشخیص و حذف خودکار سایه‌های صحافی
- **حذف نشانگر** - تشخیص و حذف علامت‌های ماژیک
- **رفع تاری** - افزایش وضوح تصاویر تار (Unsharp Mask / NAFNet / DeblurGAN-v2)
- **تصحیح رنگ** - سرکوب نفوذ رنگ HSV، سفیدسازی کاغذ
- **رابط وب** - عملکرد بصری از طریق مرورگر

---

## شروع سریع

<div dir="ltr">

```bash
# ساخت از کد منبع
git clone https://github.com/sfuruki/Rust_DN_SuperBook_Reforge.git
cd Rust_DN_SuperBook_Reforge/superbook-pdf
cargo build --release --features web

# تبدیل پایه
superbook-pdf convert input.pdf -o output/

# تبدیل با کیفیت بالا
superbook-pdf convert input.pdf -o output/ --advanced --ocr

# تبدیل به Markdown
superbook-pdf markdown input.pdf -o markdown_output/

# راه‌اندازی رابط وب
docker compose up -d
```

</div>

---

## دستورات

| دستور | توضیح |
|-------|-------|
| `convert` | بهبود PDF با AI |
| `markdown` | تولید Markdown ساختارمند از PDF |
| `reprocess` | پردازش مجدد صفحات ناموفق |
| `info` | نمایش اطلاعات محیط سیستم |
| `cache-info` | نمایش اطلاعات کش PDF خروجی |

---

## خط لوله پردازش

<div dir="ltr">

```
PDF ورودی
  |
  +- مرحله ۱:  استخراج تصاویر (pdftoppm)
  +- مرحله ۲:  برش حاشیه (پیش‌فرض ۰.۷٪)
  +- مرحله ۳:  حذف سایه
  +- مرحله ۴:  فوق‌وضوح AI (RealESRGAN 2x)
  +- مرحله ۵:  رفع تاری
  +- مرحله ۶:  تشخیص چرخش ۱۸۰ درجه
  +- مرحله ۷:  تصحیح انحراف (دوتایی‌سازی اوتسو + تبدیل هاف)
  +- مرحله ۸:  تصحیح رنگ (سرکوب نفوذ HSV)
  +- مرحله ۹:  حذف نشانگر
  +- مرحله ۱۰: برش گروهی (حاشیه‌های یکنواخت)
  +- مرحله ۱۱: تولید PDF (فشرده‌سازی JPEG DCT)
  +- مرحله ۱۲: OCR (YomiToku)
  |
  PDF خروجی
```

</div>

صفحات خالی به‌طور خودکار شناسایی می‌شوند (آستانه ۲٪) و تمام مراحل پردازش رد می‌شوند.

---

## جزئیات دستورات

### `convert` — بهبود PDF

<div dir="ltr">

```bash
# پایه
superbook-pdf convert input.pdf -o output/

# بالاترین کیفیت (همه امکانات)
superbook-pdf convert input.pdf -o output/ --advanced --ocr

# حذف سایه + نشانگر + رفع تاری
superbook-pdf convert input.pdf -o output/ --shadow-removal auto --remove-markers --deblur

# آزمایشی (5 صفحه اول، نمایش برنامه)
superbook-pdf convert input.pdf -o output/ --max-pages 5 --dry-run
```

</div>

**گزینه‌های اصلی:**

| گزینه | پیش‌فرض | توضیح |
|-------|---------|-------|
| `-o, --output <DIR>` | `./output` | پوشه خروجی |
| `--advanced` | خاموش | پردازش با کیفیت بالا (رزولوشن داخلی + تصحیح رنگ + تراز) |
| `--ocr` | خاموش | OCR ژاپنی |
| `--dpi <N>` | 300 | DPI خروجی |
| `--jpeg-quality <N>` | 90 | کیفیت فشرده‌سازی JPEG در PDF (1-100) |
| `-m, --margin-trim <N>` | 0.7 | درصد برش حاشیه (%) |
| `--shadow-removal <MODE>` | auto | حالت حذف سایه (none/auto/left/right/both) |
| `--remove-markers` | خاموش | حذف علامت‌های ماژیک |
| `--deblur` | خاموش | رفع تاری |
| `--no-upscale` | — | رد کردن فوق‌وضوح AI |
| `--no-deskew` | — | رد کردن تصحیح انحراف |
| `--no-gpu` | — | غیرفعال کردن GPU |
| `--dry-run` | — | فقط نمایش برنامه (بدون پردازش) |
| `--max-pages <N>` | — | محدود کردن تعداد صفحات |

### `markdown` — تبدیل PDF به Markdown

<div dir="ltr">

```bash
superbook-pdf markdown input.pdf -o output/
superbook-pdf markdown input.pdf -o output/ --text-direction vertical --upscale
superbook-pdf markdown input.pdf -o output/ --resume
```

</div>

**گزینه‌های اصلی:**

| گزینه | پیش‌فرض | توضیح |
|-------|---------|-------|
| `-o, --output <DIR>` | `./markdown_output` | پوشه خروجی |
| `--text-direction` | auto | جهت متن (auto/horizontal/vertical) |
| `--upscale` | خاموش | فوق‌وضوح AI قبل از OCR |
| `--dpi <N>` | 300 | DPI خروجی |
| `--figure-sensitivity <N>` | — | حساسیت تشخیص تصویر (0.0-1.0) |
| `--no-extract-images` | — | غیرفعال کردن استخراج تصویر |
| `--no-detect-tables` | — | غیرفعال کردن تشخیص جدول |
| `--validate` | خاموش | بررسی کیفیت Markdown خروجی |
| `--resume` | — | ادامه پردازش متوقف‌شده |

### `reprocess` — پردازش مجدد صفحات ناموفق

<div dir="ltr">

```bash
superbook-pdf reprocess output/.superbook-state.json
superbook-pdf reprocess output/.superbook-state.json -p 5,12,30
superbook-pdf reprocess output/.superbook-state.json --status
```

</div>

---

## نصب

### پیش‌نیازها

| مؤلفه | نیاز |
|-------|------|
| سیستم‌عامل | Linux / macOS / Windows |
| Rust | 1.82+ (برای ساخت از سورس) |
| Poppler | دستور `pdftoppm` |

ویژگی‌های AI به Python 3.10+ و GPU NVIDIA (CUDA 11.8+) نیاز دارند.

### ۱. وابستگی‌های سیستم

<div dir="ltr">

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

</div>

### ۲. نصب superbook-pdf

<div dir="ltr">

```bash
git clone https://github.com/clearclown/Rust_DN_SuperBook_PDF_Converter.git
cd Rust_DN_SuperBook_PDF_Converter/superbook-pdf
cargo build --release --features web
```

</div>

### ۳. اجرا از طریق Docker/Podman (پیشنهادی)

<div dir="ltr">

```bash
# NVIDIA GPU
docker compose up -d

# AMD GPU (ROCm)
docker compose -f docker-compose.yml -f docker-compose.rocm.yml up -d

# فقط CPU
docker compose -f docker-compose.yml -f docker-compose.cpu.yml up -d
```

</div>

آدرس http://localhost:8080 را در مرورگر باز کنید.

---

## رابط وب

![رابط وب](../doc_img/webUI.png)

رابط مرورگری که با کشیدن و رها کردن فایل‌ها تبدیل را شروع می‌کند. پشتیبانی از نمایش پیشرفت زنده از طریق WebSocket.

<div dir="ltr">

```bash
# پیشنهادی: فرانت‌اند (Nginx) + بک‌اند (Rust API/WS)
docker compose up -d

# حالت مستقیم: فقط سرور API/WS
superbook-pdf serve --port 8080 --bind 0.0.0.0
```

</div>

---

## مستندات

| سند | محتوا |
|-----|-------|
| [docs/pipeline.md](../pipeline.md) | طراحی تفصیلی خط لوله پردازش |
| [docs/commands.md](../commands.md) | مرجع کامل دستورات و گزینه‌ها |
| [docs/configuration.md](../configuration.md) | سفارشی‌سازی از طریق فایل‌های پیکربندی (TOML) |
| [docs/docker.md](../docker.md) | راهنمای تفصیلی محیط Docker/Podman |
| [docs/development.md](../development.md) | راهنمای توسعه‌دهنده (ساخت، تست، معماری) |

---

## رفع اشکال

| مشکل | راه‌حل |
|------|--------|
| `pdftoppm: command not found` | `sudo apt install poppler-utils` |
| RealESRGAN کار نمی‌کند | سرویس‌های AI را با `docker compose ps` و `superbook-pdf info` بررسی کنید |
| GPU استفاده نمی‌شود | `docker compose ps` و `nvidia-smi` را بررسی کنید؛ در صورت نیاز از حالت CPU استفاده کنید (`-f docker-compose.cpu.yml`) |
| کمبود حافظه | از `--max-pages 10` یا `--chunk-size 5` استفاده کنید |
| Deskew تصویر را تحریف می‌کند | با `--no-deskew` غیرفعال کنید |
| حاشیه‌ها متن را می‌برند | بافر ایمنی را افزایش دهید: `--margin-safety 1.0` |

---

## مجوز

AGPL v3.0 — [LICENSE](../../LICENSE)

---

## تشکر و قدردانی

- **Daiyuu Nobori** — پیاده‌سازی اصلی
- **[RealESRGAN](https://github.com/xinntao/Real-ESRGAN)** — فوق‌وضوح AI
- **[YomiToku](https://github.com/kotaro-kinoshita/yomitoku)** — OCR ژاپنی

</div>
  +- مرحله ۱۰: برش گروهی (حاشیه‌های یکنواخت)
  +- مرحله ۱۱: تولید PDF (فشرده‌سازی JPEG DCT)
  +- مرحله ۱۲: OCR (YomiToku)
  |
  PDF خروجی
```

</div>

---

## نصب

### Docker/Podman (توصیه شده)

<div dir="ltr">

```bash
# NVIDIA GPU
docker compose up -d

# AMD GPU (ROCm)
docker compose -f docker-compose.yml -f docker-compose.rocm.yml up -d

# فقط CPU
docker compose -f docker-compose.yml -f docker-compose.cpu.yml up -d
```

</div>

آدرس http://localhost:8080 را در مرورگر باز کنید.

---

## مجوز

AGPL v3.0 - [LICENSE](../../LICENSE)

## قدردانی

- **登 大遊 (Daiyuu Nobori)** - پیاده‌سازی اصلی
- **[RealESRGAN](https://github.com/xinntao/Real-ESRGAN)** - فوق‌وضوح AI
- **[YomiToku](https://github.com/kotaro-kinoshita/yomitoku)** - OCR ژاپنی

</div>
