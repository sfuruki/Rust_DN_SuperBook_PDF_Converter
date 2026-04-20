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
[![CI](https://github.com/clearclown/Rust_DN_SuperBook_PDF_Converter/actions/workflows/ci.yml/badge.svg)](https://github.com/clearclown/Rust_DN_SuperBook_PDF_Converter/actions/workflows/ci.yml)

> **فورک از [dnobori/DN_SuperBook_PDF_Converter](https://github.com/dnobori/DN_SuperBook_PDF_Converter)**
>
> ابزار بهبود کیفیت PDF کتاب‌های اسکن شده، کاملاً بازنویسی شده با Rust

**نویسنده اصلی:** دایو نوبوری (登 大遊)
**بازنویسی Rust:** clearclown
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
- **فوق‌وضوح AI** - بزرگ‌نمایی ۲ برابری با RealESRGAN
- **OCR ژاپنی** - تشخیص متن با دقت بالا توسط YomiToku
- **تبدیل به Markdown** - تولید Markdown ساختارمند از PDF (با تشخیص خودکار تصاویر و جداول)
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
git clone https://github.com/clearclown/Rust_DN_SuperBook_PDF_Converter.git
cd Rust_DN_SuperBook_PDF_Converter/superbook-pdf
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
