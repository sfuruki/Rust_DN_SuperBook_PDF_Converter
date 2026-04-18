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
[![CI](https://github.com/clearclown/Rust_DN_SuperBook_PDF_Converter/actions/workflows/ci.yml/badge.svg)](https://github.com/clearclown/Rust_DN_SuperBook_PDF_Converter/actions/workflows/ci.yml)

> **فورك من [dnobori/DN_SuperBook_PDF_Converter](https://github.com/dnobori/DN_SuperBook_PDF_Converter)**
>
> أداة تحسين جودة PDF للكتب الممسوحة ضوئياً، أُعيدت كتابتها بالكامل بلغة Rust

**المؤلف الأصلي:** دايو نوبوري (登 大遊)
**إعادة الكتابة بـ Rust:** clearclown
**الترخيص:** AGPL v3.0

---

## قبل / بعد

![مقارنة قبل وبعد](../../doc_img/ba.png)

| | قبل (يسار) | بعد (يمين) |
|---|---|---|
| **الدقة** | 1242×2048 بكسل | 2363×3508 بكسل |
| **حجم الملف** | 981 كيلوبايت | 1.6 ميغابايت |
| **الجودة** | ضبابي، تباين منخفض | واضح، تباين عالٍ |

الدقة الفائقة بالذكاء الاصطناعي عبر RealESRGAN تجعل حواف النص حادة وتحسّن القراءة بشكل ملحوظ.

---

## الميزات

- **تنفيذ بـ Rust** - إعادة كتابة كاملة من C#. تحسين كبير في كفاءة الذاكرة والأداء
- **دقة فائقة بالذكاء الاصطناعي** - تكبير مضاعف باستخدام RealESRGAN
- **التعرف الضوئي على الحروف اليابانية** - تعرف عالي الدقة باستخدام YomiToku
- **التحويل إلى Markdown** - توليد Markdown منظم من PDF (مع الكشف التلقائي عن الأشكال والجداول)
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
git clone https://github.com/clearclown/Rust_DN_SuperBook_PDF_Converter.git
cd Rust_DN_SuperBook_PDF_Converter/superbook-pdf
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
