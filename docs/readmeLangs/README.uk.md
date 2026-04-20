<p align="center">
  <b>🌐 Мова</b><br>
  <a href="../../README.md">日本語</a> |
  <a href="README.en.md">English</a> |
  <a href="README.zh-CN.md">简体中文</a> |
  <a href="README.zh-TW.md">繁體中文</a> |
  <a href="README.ru.md">Русский</a> |
  <b>Українська</b> |
  <a href="README.fa.md">فارسی</a> |
  <a href="README.ar.md">العربية</a>
</p>

# superbook-pdf

[![License: AGPL v3](https://img.shields.io/badge/License-AGPL%20v3-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)
[![Rust](https://img.shields.io/badge/rust-1.82%2B-orange.svg)](https://www.rust-lang.org/)
[![CI](https://github.com/clearclown/Rust_DN_SuperBook_PDF_Converter/actions/workflows/ci.yml/badge.svg)](https://github.com/clearclown/Rust_DN_SuperBook_PDF_Converter/actions/workflows/ci.yml)

> **Форк [dnobori/DN_SuperBook_PDF_Converter](https://github.com/dnobori/DN_SuperBook_PDF_Converter)**
>
> Інструмент для високоякісного покращення PDF відсканованих книг, повністю переписаний на Rust

**Оригінальний автор:** Дайю Нобори (登 大遊)
**Переписаний на Rust:** clearclown
**Ліцензія:** AGPL v3.0

---

## До / Після

![Порівняння до та після](../doc_img/ba.png)

| | До (зліва) | Після (справа) |
|---|---|---|
| **Роздільна здатність** | 1242x2048 px | 2363x3508 px |
| **Розмір файлу** | 981 КБ | 1.6 МБ |
| **Якість** | Розмите, низький контраст | Чітке, високий контраст |

AI-суперроздільність з RealESRGAN робить краї тексту чіткими та значно покращує читабельність.

---

## Можливості

- **Реалізація на Rust** - Повністю переписаний з C#. Значно покращено ефективність використання пам'яті та продуктивність
- **AI-суперроздільність** - 2-кратне збільшення за допомогою RealESRGAN
- **Японське OCR** - Високоточне розпізнавання тексту з YomiToku
- **Конвертація в Markdown** - Генерація структурованого Markdown з PDF (автоматичне виявлення рисунків та таблиць)
- **Корекція нахилу** - Автоматична корекція через бінаризацію Оцу + перетворення Хафа
- **Виявлення повороту на 180°** - Автоматичне виявлення та виправлення перевернутих сторінок
- **Видалення тіней** - Автоматичне виявлення та видалення тіней від палітурки
- **Видалення маркерів** - Виявлення та видалення виділень маркером
- **Усунення розмиття** - Підвищення різкості розмитих зображень (Unsharp Mask / NAFNet / DeblurGAN-v2)
- **Корекція кольору** - Придушення просвічування HSV, відбілювання паперу
- **Веб-інтерфейс** - Інтуїтивне керування через браузер

---

## Швидкий старт

```bash
# Збірка з вихідного коду
git clone https://github.com/clearclown/Rust_DN_SuperBook_PDF_Converter.git
cd Rust_DN_SuperBook_PDF_Converter/superbook-pdf
cargo build --release --features web

# Базова конвертація
superbook-pdf convert input.pdf -o output/

# Високоякісна конвертація
superbook-pdf convert input.pdf -o output/ --advanced --ocr

# Конвертація в Markdown
superbook-pdf markdown input.pdf -o markdown_output/

# Запуск веб-інтерфейсу
docker compose up -d
```

---

## Команди

| Команда | Опис |
|---------|------|
| `convert` | AI-покращення PDF |
| `markdown` | Генерація Markdown з PDF |
| `reprocess` | Повторна обробка невдалих сторінок |
| `info` | Інформація про системне середовище |
| `cache-info` | Інформація про кеш вихідного PDF |

---

## Конвеєр обробки

```
Вхідний PDF
  |
  +- Крок 1:  Вилучення зображень (pdftoppm)
  +- Крок 2:  Обрізка полів (за замовч. 0.7%)
  +- Крок 3:  Видалення тіней
  +- Крок 4:  AI-суперроздільність (RealESRGAN 2x)
  +- Крок 5:  Усунення розмиття
  +- Крок 6:  Виявлення повороту на 180°
  +- Крок 7:  Корекція нахилу (бінаризація Оцу + перетв. Хафа)
  +- Крок 8:  Корекція кольору (придушення просвічування HSV)
  +- Крок 9:  Видалення маркерів
  +- Крок 10: Групове обрізання (однорідні поля)
  +- Крок 11: Генерація PDF (стиснення JPEG DCT)
  +- Крок 12: OCR (YomiToku)
  |
  Вихідний PDF
```

---

## Встановлення

### Docker/Podman (рекомендовано)

```bash
# NVIDIA GPU
docker compose up -d

# AMD GPU (ROCm)
docker compose -f docker-compose.yml -f docker-compose.rocm.yml up -d

# Тільки CPU
docker compose -f docker-compose.yml -f docker-compose.cpu.yml up -d
```

Відкрийте http://localhost:8080 у браузері.

---

## Ліцензія

AGPL v3.0 - [LICENSE](../../LICENSE)

## Подяки

- **登 大遊 (Daiyuu Nobori)** - Оригінальна реалізація
- **[RealESRGAN](https://github.com/xinntao/Real-ESRGAN)** - AI-суперроздільність
- **[YomiToku](https://github.com/kotaro-kinoshita/yomitoku)** - Японське OCR
