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
[![CI](https://github.com/sfuruki/Rust_DN_SuperBook_Reforge/actions/workflows/ci.yml/badge.svg)](https://github.com/sfuruki/Rust_DN_SuperBook_Reforge/actions/workflows/ci.yml)

> **Линия форков:**
>
> - [dnobori/DN_SuperBook_PDF_Converter](https://github.com/dnobori/DN_SuperBook_PDF_Converter) (оригинал)
> - [clearclown/Rust_DN_SuperBook_PDF_Converter](https://github.com/clearclown/Rust_DN_SuperBook_PDF_Converter) (Rust-форк)
>
> Rust_DN_SuperBook_Reforge — производный проект, продолжающий линию DN_SuperBook_PDF_Converter и Rust_DN_SuperBook_PDF_Converter.
>
> Он сохраняет основные возможности конвертации, но перерабатывает структуру запуска и эксплуатации под текущие среды, чтобы упростить развитие и сопровождение.
>
> В этом производном варианте основное внимание уделено разделению AI-окружения и HTTP-микросервисам, частичному параллельному выполнению по страницам и более подробному отображению прогресса в Web UI / WebSocket.

**Оригинальный автор:** Дайю Нобори (登 大遊)
**Переписан на Rust:** clearclown
**Производная версия / адаптация:** sfuruki
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
- **Разделённое AI-окружение** - RealESRGAN / YomiToku отделены от Rust Core и работают как HTTP-микросервисы через Docker/Podman
- **AI-суперразрешение** - 2-кратное увеличение с помощью RealESRGAN
- **Японское OCR** - Высокоточное распознавание текста с YomiToku
- **Конвертация в Markdown** - Генерация структурированного Markdown из PDF (автоматическое обнаружение рисунков и таблиц)
- **Частичное параллельное выполнение** - Постраничная параллельная обработка с управлением нагрузкой и памятью через `--threads` и `--chunk-size`
- **Детализированный прогресс** - Улучшено отображение этапов обработки и логов поверх существующей связки Web UI / WebSocket
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
git clone https://github.com/sfuruki/Rust_DN_SuperBook_Reforge.git
cd Rust_DN_SuperBook_Reforge/superbook-pdf
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
  +- Шаг 10: Групповое кадрирование (единые поля)
  +- Шаг 11: Генерация PDF (JPEG DCT сжатие)
  +- Шаг 12: OCR (YomiToku)
  |
  Выходной PDF
```

Пустые страницы автоматически определяются (порог 2%) и пропускают всю обработку.

---

## Команды

### `convert` — Улучшение PDF

```bash
# Базовое (коррекция наклона + обрезка полей + AI-суперразрешение)
superbook-pdf convert input.pdf -o output/

# Наилучшее качество (все функции включены)
superbook-pdf convert input.pdf -o output/ --advanced --ocr

# Удаление теней + маркеров + устранение размытия
superbook-pdf convert input.pdf -o output/ --shadow-removal auto --remove-markers --deblur

# Тест (первые 5 страниц, только план)
superbook-pdf convert input.pdf -o output/ --max-pages 5 --dry-run
```

**Основные параметры:**

| Параметр | По умолчанию | Описание |
|----------|-------------|----------|
| `-o, --output <DIR>` | `./output` | Выходной каталог |
| `--advanced` | выкл | Высококачественная обработка (внутреннее разрешение + цветокоррекция + выравнивание) |
| `--ocr` | выкл | Японское OCR |
| `--dpi <N>` | 300 | Выходное DPI |
| `--jpeg-quality <N>` | 90 | Качество JPEG сжатия в PDF (1-100) |
| `-m, --margin-trim <N>` | 0.7 | Процент обрезки полей (%) |
| `--shadow-removal <MODE>` | auto | Режим удаления теней (none/auto/left/right/both) |
| `--remove-markers` | выкл | Удаление маркеров-выделителей |
| `--deblur` | выкл | Устранение размытия |
| `--no-upscale` | — | Пропустить AI-суперразрешение |
| `--no-deskew` | — | Пропустить коррекцию наклона |
| `--no-gpu` | — | Отключить GPU |
| `--dry-run` | — | Только план выполнения (без обработки) |
| `--max-pages <N>` | — | Ограничение числа страниц |

### `markdown` — Конвертация PDF в Markdown

```bash
# Базовая конвертация
superbook-pdf markdown input.pdf -o output/

# Вертикальный текст + AI-суперразрешение
superbook-pdf markdown input.pdf -o output/ --text-direction vertical --upscale

# Возобновить прерванную обработку
superbook-pdf markdown input.pdf -o output/ --resume
```

**Основные параметры:**

| Параметр | По умолчанию | Описание |
|----------|-------------|----------|
| `-o, --output <DIR>` | `./markdown_output` | Выходной каталог |
| `--text-direction` | auto | Направление текста (auto/horizontal/vertical) |
| `--upscale` | выкл | AI-суперразрешение перед OCR |
| `--dpi <N>` | 300 | Выходное DPI |
| `--figure-sensitivity <N>` | — | Чувствительность обнаружения рисунков (0.0-1.0) |
| `--no-extract-images` | — | Отключить извлечение изображений |
| `--no-detect-tables` | — | Отключить обнаружение таблиц |
| `--validate` | выкл | Проверка качества выходного Markdown |
| `--resume` | — | Возобновить прерванную обработку |

### `reprocess` — Повторная обработка неудавшихся страниц

```bash
# Автоопределение и повторная обработка из файла состояния
superbook-pdf reprocess output/.superbook-state.json

# Только определённые страницы
superbook-pdf reprocess output/.superbook-state.json -p 5,12,30

# Только просмотр статуса
superbook-pdf reprocess output/.superbook-state.json --status
```

---

## Установка

### Требования

| Компонент | Требование |
|-----------|------------|
| ОС | Linux / macOS / Windows |
| Rust | 1.82+ (для сборки из исходников) |
| Poppler | команда `pdftoppm` |

Для AI-функций требуются Python 3.10+ и GPU NVIDIA (CUDA 11.8+).

### 1. Системные зависимости

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

### 2. Установка superbook-pdf

```bash
git clone https://github.com/clearclown/Rust_DN_SuperBook_PDF_Converter.git
cd Rust_DN_SuperBook_PDF_Converter/superbook-pdf
cargo build --release --features web
```

### 3. Запуск через Docker/Podman (рекомендуется)

```bash
# GPU NVIDIA
docker compose up -d

# GPU AMD (ROCm)
docker compose -f docker-compose.yml -f docker-compose.rocm.yml up -d

# Только CPU
docker compose -f docker-compose.yml -f docker-compose.cpu.yml up -d
```

Откройте http://localhost:8080 в браузере.

---

## Веб-интерфейс

![Веб-интерфейс](../doc_img/webUI.png)

Браузерный интерфейс с перетаскиванием файлов для начала конвертации. Поддержка отображения прогресса в реальном времени через WebSocket.

```bash
# Рекомендуется: фронтенд (Nginx) + бэкенд (Rust API/WS)
docker compose up -d

# Прямой режим: только сервер API/WS
superbook-pdf serve --port 8080 --bind 0.0.0.0
```

---

## Документация

| Документ | Содержание |
|----------|------------|
| [docs/pipeline.md](../pipeline.md) | Детальный дизайн конвейера обработки |
| [docs/commands.md](../commands.md) | Полный справочник команд и параметров |
| [docs/configuration.md](../configuration.md) | Настройка через конфигурационные файлы (TOML) |
| [docs/docker.md](../docker.md) | Подробное руководство по Docker/Podman |
| [docs/development.md](../development.md) | Руководство разработчика (сборка, тесты, архитектура) |

---

## Устранение неполадок

| Проблема | Решение |
|----------|----------|
| `pdftoppm: command not found` | `sudo apt install poppler-utils` |
| RealESRGAN не работает | Проверьте AI-сервисы: `docker compose ps` и `superbook-pdf info` |
| GPU не используется | Проверьте `docker compose ps` и `nvidia-smi`; при необходимости используйте CPU (`-f docker-compose.cpu.yml`) |
| Нехватка памяти | Используйте `--max-pages 10` или `--chunk-size 5` |
| Deskew искажает изображение | Отключите с `--no-deskew` |
| Поля обрезают текст | Увеличьте буфер: `--margin-safety 1.0` |

---

## Лицензия

AGPL v3.0 — [LICENSE](../../LICENSE)

---

## Благодарности

- **Daiyuu Nobori** — оригинальная реализация
- **[RealESRGAN](https://github.com/xinntao/Real-ESRGAN)** — AI-суперразрешение
- **[YomiToku](https://github.com/kotaro-kinoshita/yomitoku)** — японское OCR
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
