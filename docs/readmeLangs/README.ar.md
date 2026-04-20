<div dir="rtl">

<p align="center">
  <b>🌐 اللغة</b><br>
  <a href="../../README.md">日本語</a> |
  <a href="README.en.md">English</a> |
  <a href="README.zh-CN.md">简体中文</a> |
  <a href="README.zh-TW.md">繁體中文</a> |
  <a href="README.ru.md">Русский</a> |
  <a href="README.uk.md">Українська</a> |
  <a href="README.fa.md">فارسی</a> |
  <b>العربية</b>
</p>

# superbook-pdf

[![License: AGPL v3](https://img.shields.io/badge/License-AGPL%20v3-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)
[![Rust](https://img.shields.io/badge/rust-1.82%2B-orange.svg)](https://www.rust-lang.org/)
[![CI](https://github.com/sfuruki/Rust_DN_SuperBook_Reforge/actions/workflows/ci.yml/badge.svg)](https://github.com/sfuruki/Rust_DN_SuperBook_Reforge/actions/workflows/ci.yml)

> **سلسلة التفرعات:**
>
> - [dnobori/DN_SuperBook_PDF_Converter](https://github.com/dnobori/DN_SuperBook_PDF_Converter) (الأصل)
> - [clearclown/Rust_DN_SuperBook_PDF_Converter](https://github.com/clearclown/Rust_DN_SuperBook_PDF_Converter) (تفرع Rust)
>
> Rust_DN_SuperBook_Reforge هو مشروع مشتق يواصل مسار DN_SuperBook_PDF_Converter و Rust_DN_SuperBook_PDF_Converter.
>
> يحتفظ بميزات التحويل الأساسية، مع إعادة تنظيم بنية التشغيل والاستخدام لتناسب البيئات الحالية وتسهيل التوسعة والصيانة.
>
> يركّز هذا الإصدار المشتق خصوصاً على فصل بيئة تنفيذ الذكاء الاصطناعي وتحويلها إلى خدمات HTTP مصغّرة، والتنفيذ المتوازي الجزئي على مستوى الصفحات، وتفصيل عرض التقدم في Web UI / WebSocket.

**المؤلف الأصلي:** دايو نوبوري (登 大遊)
**إعادة الكتابة بـ Rust:** clearclown
**النسخة المشتقة / التعديلات:** sfuruki
**الترخيص:** AGPL v3.0

---

## قبل / بعد

![مقارنة قبل وبعد](../doc_img/ba.png)

| | قبل (يسار) | بعد (يمين) |
|---|---|---|
| **الدقة** | 1242×2048 بكسل | 2363×3508 بكسل |
| **حجم الملف** | 981 كيلوبايت | 1.6 ميغابايت |
| **الجودة** | ضبابي، تباين منخفض | واضح، تباين عالٍ |

الدقة الفائقة بالذكاء الاصطناعي عبر RealESRGAN تجعل حواف النص حادة وتحسّن القراءة بشكل ملحوظ.

---

## الميزات

- **تنفيذ بـ Rust** - إعادة كتابة كاملة من C#. تحسين كبير في كفاءة الذاكرة والأداء
- **فصل بيئة تنفيذ الذكاء الاصطناعي** - تم فصل RealESRGAN / YomiToku عن Rust Core وتشغيلهما كخدمات HTTP مصغّرة عبر Docker/Podman
- **دقة فائقة بالذكاء الاصطناعي** - تكبير مضاعف باستخدام RealESRGAN
- **التعرف الضوئي على الحروف اليابانية** - تعرف عالي الدقة باستخدام YomiToku
- **التحويل إلى Markdown** - توليد Markdown منظم من PDF (مع الكشف التلقائي عن الأشكال والجداول)
- **تنفيذ متوازٍ جزئي** - معالجة متوازية على مستوى الصفحات مع التحكم في الحمل والذاكرة عبر `--threads` و `--chunk-size`
- **تفصيل عرض التقدم** - تحسين عرض مراحل المعالجة والسجلات فوق البنية الحالية لـ Web UI / WebSocket
- **تصحيح الميل** - تصحيح تلقائي عبر ثنائية أوتسو + تحويل هاف
- **كشف الدوران 180 درجة** - كشف وتصحيح تلقائي للصفحات المقلوبة
- **إزالة الظلال** - كشف وإزالة تلقائية لظلال التجليد
- **إزالة العلامات** - كشف وإزالة تمييز أقلام الفلوريسنت
- **إزالة الضبابية** - شحذ الصور الضبابية (Unsharp Mask / NAFNet / DeblurGAN-v2)
- **تصحيح الألوان** - كبت النفاذ HSV، تبييض الورق
- **واجهة ويب** - تشغيل بديهي عبر المتصفح

---

## البدء السريع

<div dir="ltr">

```bash
# البناء من المصدر
git clone https://github.com/sfuruki/Rust_DN_SuperBook_Reforge.git
cd Rust_DN_SuperBook_Reforge/superbook-pdf
cargo build --release --features web

# تحويل أساسي
superbook-pdf convert input.pdf -o output/

# تحويل عالي الجودة
superbook-pdf convert input.pdf -o output/ --advanced --ocr

# التحويل إلى Markdown
superbook-pdf markdown input.pdf -o markdown_output/

# تشغيل واجهة الويب
docker compose up -d
```

</div>

---

## الأوامر

| الأمر | الوصف |
|-------|-------|
| `convert` | تحسين PDF بالذكاء الاصطناعي |
| `markdown` | توليد Markdown منظم من PDF |
| `reprocess` | إعادة معالجة الصفحات الفاشلة |
| `info` | عرض معلومات بيئة النظام |
| `cache-info` | عرض معلومات ذاكرة التخزين المؤقت لـ PDF الناتج |

---

## خط أنابيب المعالجة

<div dir="ltr">

```
PDF المدخل
  |
  +- الخطوة 1:  استخراج الصور (pdftoppm)
  +- الخطوة 2:  قص الهوامش (افتراضي 0.7%)
  +- الخطوة 3:  إزالة الظلال
  +- الخطوة 4:  الدقة الفائقة (RealESRGAN 2x)
  +- الخطوة 5:  إزالة الضبابية
  +- الخطوة 6:  كشف الدوران 180 درجة
  +- الخطوة 7:  تصحيح الميل (ثنائية أوتسو + تحويل هاف)
  +- الخطوة 8:  تصحيح الألوان (كبت النفاذ HSV)
  +- الخطوة 9:  إزالة العلامات
  +- الخطوة 10: قص موحد للهوامش
  +- الخطوة 11: إنشاء PDF (ضغط JPEG DCT)
  +- الخطوة 12: OCR (YomiToku)
  |
  PDF المخرج
```

</div>

تُكتشف الصفحات الفارغة تلقائياً (عتبة 2%) وتُتجاوز جميع خطوات المعالجة.

---

## تفاصيل الأوامر

### `convert` — تحسين PDF

<div dir="ltr">

```bash
# أساسي
superbook-pdf convert input.pdf -o output/

# أعلى جودة (جميع الميزات)
superbook-pdf convert input.pdf -o output/ --advanced --ocr

# إزالة الظلال + العلامات + الضبابية
superbook-pdf convert input.pdf -o output/ --shadow-removal auto --remove-markers --deblur

# اختبار (5 صفحات فقط، عرض الخطة)
superbook-pdf convert input.pdf -o output/ --max-pages 5 --dry-run
```

</div>

**الخيارات الرئيسية:**

| الخيار | الافتراضي | الوصف |
|--------|-----------|-------|
| `-o, --output <DIR>` | `./output` | مجلد الإخراج |
| `--advanced` | إيقاف | معالجة عالية الجودة (دقة داخلية + تصحيح ألوان + محاذاة) |
| `--ocr` | إيقاف | OCR ياباني |
| `--dpi <N>` | 300 | دقة الإخراج |
| `--jpeg-quality <N>` | 90 | جودة ضغط JPEG في PDF (1-100) |
| `-m, --margin-trim <N>` | 0.7 | نسبة قص الهوامش (%) |
| `--shadow-removal <MODE>` | auto | وضع إزالة الظلال (none/auto/left/right/both) |
| `--remove-markers` | إيقاف | إزالة علامات الفلوريسنت |
| `--deblur` | إيقاف | إزالة الضبابية |
| `--no-upscale` | — | تخطي الدقة الفائقة |
| `--no-deskew` | — | تخطي تصحيح الميل |
| `--no-gpu` | — | تعطيل GPU |
| `--dry-run` | — | عرض الخطة فقط (بدون معالجة) |
| `--max-pages <N>` | — | تحديد عدد الصفحات |

### `markdown` — تحويل PDF إلى Markdown

<div dir="ltr">

```bash
superbook-pdf markdown input.pdf -o output/
superbook-pdf markdown input.pdf -o output/ --text-direction vertical --upscale
superbook-pdf markdown input.pdf -o output/ --resume
```

</div>

**الخيارات الرئيسية:**

| الخيار | الافتراضي | الوصف |
|--------|-----------|-------|
| `-o, --output <DIR>` | `./markdown_output` | مجلد الإخراج |
| `--text-direction` | auto | اتجاه النص (auto/horizontal/vertical) |
| `--upscale` | إيقاف | دقة فائقة قبل OCR |
| `--dpi <N>` | 300 | دقة الإخراج |
| `--figure-sensitivity <N>` | — | حساسية كشف الأشكال (0.0-1.0) |
| `--no-extract-images` | — | تعطيل استخراج الصور |
| `--no-detect-tables` | — | تعطيل كشف الجداول |
| `--validate` | إيقاف | التحقق من جودة Markdown المخرج |
| `--resume` | — | استئناف المعالجة المتوقفة |

### `reprocess` — إعادة معالجة الصفحات الفاشلة

<div dir="ltr">

```bash
superbook-pdf reprocess output/.superbook-state.json
superbook-pdf reprocess output/.superbook-state.json -p 5,12,30
superbook-pdf reprocess output/.superbook-state.json --status
```

</div>

---

## التثبيت

### المتطلبات

| المكون | المتطلب |
|--------|---------|
| نظام التشغيل | Linux / macOS / Windows |
| Rust | 1.82+ (للبناء من المصدر) |
| Poppler | أمر `pdftoppm` |

ميزات الذكاء الاصطناعي تتطلب Python 3.10+ و GPU NVIDIA (CUDA 11.8+).

### 1. التبعيات النظامية

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

### 2. تثبيت superbook-pdf

<div dir="ltr">

```bash
git clone https://github.com/clearclown/Rust_DN_SuperBook_PDF_Converter.git
cd Rust_DN_SuperBook_PDF_Converter/superbook-pdf
cargo build --release --features web
```

</div>

### 3. التشغيل عبر Docker/Podman (موصى به)

<div dir="ltr">

```bash
# NVIDIA GPU
docker compose up -d

# AMD GPU (ROCm)
docker compose -f docker-compose.yml -f docker-compose.rocm.yml up -d

# CPU فقط
docker compose -f docker-compose.yml -f docker-compose.cpu.yml up -d
```

</div>

افتح http://localhost:8080 في المتصفح.

---

## واجهة الويب

![واجهة الويب](../doc_img/webUI.png)

واجهة مستعرض تتيح سحب الملفات وإفلاتها لبدء التحويل. دعم لعرض التقدم في الوقت الفعلي عبر WebSocket.

<div dir="ltr">

```bash
# موصى به: واجهة أمامية (Nginx) + خلفية (Rust API/WS)
docker compose up -d

# الوضع المباشر: خادم API/WS فقط
superbook-pdf serve --port 8080 --bind 0.0.0.0
```

</div>

---

## التوثيق

| الوثيقة | المحتوى |
|---------|---------|
| [docs/pipeline.md](../pipeline.md) | تصميم تفصيلي لخط المعالجة |
| [docs/commands.md](../commands.md) | مرجع كامل للأوامر والخيارات |
| [docs/configuration.md](../configuration.md) | التخصيص عبر ملفات الإعداد (TOML) |
| [docs/docker.md](../docker.md) | دليل بيئة Docker/Podman التفصيلي |
| [docs/development.md](../development.md) | دليل المطورين (البناء، الاختبارات، البنية) |

---

## استكشاف الأخطاء

| المشكلة | الحل |
|---------|------|
| `pdftoppm: command not found` | `sudo apt install poppler-utils` |
| RealESRGAN لا يعمل | تحقق من خدمات الذكاء الاصطناعي: `docker compose ps` و `superbook-pdf info` |
| GPU غير مستخدم | تحقق من `docker compose ps` و `nvidia-smi`؛ استخدم وضع CPU إذا لزم (`-f docker-compose.cpu.yml`) |
| نفاد الذاكرة | استخدم `--max-pages 10` أو `--chunk-size 5` |
| Deskew يشوه الصورة | عطّل بـ `--no-deskew` |
| الهوامش تقطع النص | زد المخزن المؤقت: `--margin-safety 1.0` |

---

## الترخيص

AGPL v3.0 — [LICENSE](../../LICENSE)

---

## شكر وتقدير

- **Daiyuu Nobori** — التنفيذ الأصلي
- **[RealESRGAN](https://github.com/xinntao/Real-ESRGAN)** — الدقة الفائقة بالذكاء الاصطناعي
- **[YomiToku](https://github.com/kotaro-kinoshita/yomitoku)** — OCR الياباني

</div>
  +- الخطوة 10: القص الجماعي (هوامش موحدة)
  +- الخطوة 11: توليد PDF (ضغط JPEG DCT)
  +- الخطوة 12: OCR (YomiToku)
  |
  PDF الناتج
```

</div>

---

## التثبيت

### Docker/Podman (موصى به)

<div dir="ltr">

```bash
# NVIDIA GPU
docker compose up -d

# AMD GPU (ROCm)
docker compose -f docker-compose.yml -f docker-compose.rocm.yml up -d

# CPU فقط
docker compose -f docker-compose.yml -f docker-compose.cpu.yml up -d
```

</div>

افتح http://localhost:8080 في المتصفح.

---

## الترخيص

AGPL v3.0 - [LICENSE](../../LICENSE)

## شكر وتقدير

- **登 大遊 (Daiyuu Nobori)** - التنفيذ الأصلي
- **[RealESRGAN](https://github.com/xinntao/Real-ESRGAN)** - الدقة الفائقة بالذكاء الاصطناعي
- **[YomiToku](https://github.com/kotaro-kinoshita/yomitoku)** - التعرف الضوئي على الحروف اليابانية

</div>
