<p align="center">
  <b>🌐 Язык</b><br>
  <a href="../../README.md">日本語</a> |
  <a href="README.en.md">English</a> |
  <a href="README.zh-CN.md">简体中文</a> |
  <a href="README.zh-TW.md">繁體中文</a> |
  <b>Русский</b> |
  <a href="README.uk.md">Українська</a> |
  <a href="README.fa.md">فارسی</a> |
  <a href="README.ar.md">العربية</a>
</p>

# superbook-pdf

[![License: AGPL v3](https://img.shields.io/badge/License-AGPL%20v3-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)
[![Rust](https://img.shields.io/badge/rust-1.82%2B-orange.svg)](https://www.rust-lang.org/)
[![CI](https://github.com/clearclown/Rust_DN_SuperBook_PDF_Converter/actions/workflows/ci.yml/badge.svg)](https://github.com/clearclown/Rust_DN_SuperBook_PDF_Converter/actions/workflows/ci.yml)

> **Форк [dnobori/DN_SuperBook_PDF_Converter](https://github.com/dnobori/DN_SuperBook_PDF_Converter)**
>
> Инструмент для высококачественного улучшения PDF отсканированных книг, полностью переписанный на Rust

**Оригинальный автор:** Дайю Нобори (登 大遊)
**Переписан на Rust:** clearclown
**Лицензия:** AGPL v3.0

---

## До / После

![Сравнение до и после](../doc_img/ba.png)

| | До (слева) | После (справа) |
|---|---|---|
| **Разрешение** | 1242x2048 px | 2363x3508 px |
| **Размер файла** | 981 КБ | 1.6 МБ |
| **Качество** | Размытое, низкий контраст | Чёткое, высокий контраст |

AI-суперразрешение с RealESRGAN делает края текста чёткими и значительно улучшает читаемость.

---

## Возможности

- **Реализация на Rust** - Полностью переписан с C#. Значительно улучшена эффективность использования памяти и производительность
- **AI-суперразрешение** - 2-кратное увеличение с помощью RealESRGAN
- **Японское OCR** - Высокоточное распознавание текста с YomiToku
- **Конвертация в Markdown** - Генерация структурированного Markdown из PDF (автоматическое обнаружение рисунков и таблиц)
- **Коррекция наклона** - Автоматическая коррекция через бинаризацию Оцу + преобразование Хафа
- **Обнаружение поворота на 180°** - Автоматическое обнаружение и исправление перевёрнутых страниц
- **Удаление теней** - Автоматическое обнаружение и удаление теней от переплёта
- **Удаление маркеров** - Обнаружение и удаление выделений маркером
- **Устранение размытия** - Повышение резкости размытых изображений (Unsharp Mask / NAFNet / DeblurGAN-v2)
- **Цветокоррекция** - Подавление просвечивания HSV, отбеливание бумаги
- **Веб-интерфейс** - Интуитивное управление через браузер

---

## Быстрый старт

```bash
# Сборка из исходников
git clone https://github.com/clearclown/Rust_DN_SuperBook_PDF_Converter.git
cd Rust_DN_SuperBook_PDF_Converter/superbook-pdf
cargo build --release --features web

# Базовая конвертация
superbook-pdf convert input.pdf -o output/

# Высококачественная конвертация (AI-суперразрешение + цветокоррекция + выравнивание)
superbook-pdf convert input.pdf -o output/ --advanced --ocr

# Конвертация в Markdown
superbook-pdf markdown input.pdf -o markdown_output/

# Запуск веб-интерфейса
docker compose up -d
```

---

## Команды

| Команда | Описание |
|---------|----------|
| `convert` | AI-улучшение PDF для получения высококачественного PDF |
| `markdown` | Генерация структурированного Markdown из PDF |
| `reprocess` | Повторная обработка неудавшихся страниц |
| `info` | Информация о системном окружении |
| `cache-info` | Информация о кэше выходного PDF |

---

## Конвейер обработки

```
Входной PDF
  |
  +- Шаг 1:  Извлечение изображений (pdftoppm)
  +- Шаг 2:  Обрезка полей (по умолч. 0.7%)
  +- Шаг 3:  Удаление теней
  +- Шаг 4:  AI-суперразрешение (RealESRGAN 2x)
  +- Шаг 5:  Устранение размытия
  +- Шаг 6:  Обнаружение поворота на 180°
  +- Шаг 7:  Коррекция наклона (бинаризация Оцу + преобр. Хафа)
  +- Шаг 8:  Цветокоррекция (подавление просвечивания HSV)
  +- Шаг 9:  Удаление маркеров
  +- Шаг 10: Групповая обрезка (единые поля)
  +- Шаг 11: Генерация PDF (сжатие JPEG DCT)
  +- Шаг 12: OCR (YomiToku)
  |
  Выходной PDF
```

---

## Установка

### Docker/Podman (рекомендуется)

```bash
# NVIDIA GPU
docker compose up -d

# AMD GPU (ROCm)
docker compose -f docker-compose.yml -f docker-compose.rocm.yml up -d

# Только CPU
docker compose -f docker-compose.yml -f docker-compose.cpu.yml up -d
```

Откройте http://localhost:8080 в браузере.

---

## Лицензия

AGPL v3.0 - [LICENSE](../../LICENSE)

## Благодарности

- **登 大遊 (Daiyuu Nobori)** - Оригинальная реализация
- **[RealESRGAN](https://github.com/xinntao/Real-ESRGAN)** - AI-суперразрешение
- **[YomiToku](https://github.com/kotaro-kinoshita/yomitoku)** - Японское OCR
