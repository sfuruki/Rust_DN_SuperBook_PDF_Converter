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
[![CI](https://github.com/sfuruki/Rust_DN_SuperBook_Reforge/actions/workflows/ci.yml/badge.svg)](https://github.com/sfuruki/Rust_DN_SuperBook_Reforge/actions/workflows/ci.yml)

> **Лінія форків:**
>
> - [dnobori/DN_SuperBook_PDF_Converter](https://github.com/dnobori/DN_SuperBook_PDF_Converter) (оригінал)
> - [clearclown/Rust_DN_SuperBook_PDF_Converter](https://github.com/clearclown/Rust_DN_SuperBook_PDF_Converter) (Rust-форк)
>
> Rust_DN_SuperBook_Reforge — похідний проєкт, що продовжує лінію DN_SuperBook_PDF_Converter та Rust_DN_SuperBook_PDF_Converter.
>
> Він зберігає основні можливості конвертації, але перебудовує структуру запуску та експлуатації під сучасні середовища, щоб спростити подальший розвиток і підтримку.
>
> У цій похідній версії основний акцент зроблено на відокремленні AI-середовища та HTTP-мікросервісах, частковому паралельному виконанні на рівні сторінок і детальнішому відображенні прогресу у Web UI / WebSocket.

**Оригінальний автор:** Дайю Нобори (登 大遊)
**Переписаний на Rust:** clearclown
**Похідна версія / адаптація:** sfuruki
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
- **Відокремлене AI-середовище** - RealESRGAN / YomiToku відділено від Rust Core і запускається як HTTP-мікросервіси через Docker/Podman
- **AI-суперроздільність** - 2-кратне збільшення за допомогою RealESRGAN
- **Японське OCR** - Високоточне розпізнавання тексту з YomiToku
- **Конвертація в Markdown** - Генерація структурованого Markdown з PDF (автоматичне виявлення рисунків та таблиць)
- **Часткове паралельне виконання** - Паралельна обробка на рівні сторінок з керуванням навантаженням і пам'яттю через `--threads` та `--chunk-size`
- **Деталізований прогрес** - Поліпшене відображення етапів обробки та логів поверх наявної зв'язки Web UI / WebSocket
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
git clone https://github.com/sfuruki/Rust_DN_SuperBook_Reforge.git
cd Rust_DN_SuperBook_Reforge/superbook-pdf
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
  +- Крок 10: Групове кадрування (єдині поля)
  +- Крок 11: Генерація PDF (JPEG DCT стиснення)
  +- Крок 12: OCR (YomiToku)
  |
  Вихідний PDF
```

Порожні сторінки автоматично визначаються (поріг 2%) та пропускають всю обробку.

---

## Команди

### `convert` — Покращення PDF

```bash
# Базове (корекція нахилу + обрізка полів + AI-суперроздільність)
superbook-pdf convert input.pdf -o output/

# Найкраща якість (всі функції увімкнено)
superbook-pdf convert input.pdf -o output/ --advanced --ocr

# Видалення тіней + маркерів + усунення розмиття
superbook-pdf convert input.pdf -o output/ --shadow-removal auto --remove-markers --deblur

# Тест (перші 5 сторінок, тільки план)
superbook-pdf convert input.pdf -o output/ --max-pages 5 --dry-run
```

**Основні параметри:**

| Параметр | За замовч. | Опис |
|----------|-----------|------|
| `-o, --output <DIR>` | `./output` | Вихідний каталог |
| `--advanced` | викл | Високоякісна обробка (внутрішня роздільність + корекція кольору + вирівнювання) |
| `--ocr` | викл | Японське OCR |
| `--dpi <N>` | 300 | Вихідне DPI |
| `--jpeg-quality <N>` | 90 | Якість JPEG стиснення в PDF (1-100) |
| `-m, --margin-trim <N>` | 0.7 | Відсоток обрізки полів (%) |
| `--shadow-removal <MODE>` | auto | Режим видалення тіней (none/auto/left/right/both) |
| `--remove-markers` | викл | Видалення маркерів-виділювачів |
| `--deblur` | викл | Усунення розмиття |
| `--no-upscale` | — | Пропустити AI-суперроздільність |
| `--no-deskew` | — | Пропустити корекцію нахилу |
| `--no-gpu` | — | Вимкнути GPU |
| `--dry-run` | — | Тільки план виконання (без обробки) |
| `--max-pages <N>` | — | Обмеження кількості сторінок |

### `markdown` — Конвертація PDF у Markdown

```bash
# Базова конвертація
superbook-pdf markdown input.pdf -o output/

# Вертикальний текст + AI-суперроздільність
superbook-pdf markdown input.pdf -o output/ --text-direction vertical --upscale

# Відновити перервану обробку
superbook-pdf markdown input.pdf -o output/ --resume
```

**Основні параметри:**

| Параметр | За замовч. | Опис |
|----------|-----------|------|
| `-o, --output <DIR>` | `./markdown_output` | Вихідний каталог |
| `--text-direction` | auto | Напрямок тексту (auto/horizontal/vertical) |
| `--upscale` | викл | AI-суперроздільність перед OCR |
| `--dpi <N>` | 300 | Вихідне DPI |
| `--figure-sensitivity <N>` | — | Чутливість виявлення рисунків (0.0-1.0) |
| `--no-extract-images` | — | Вимкнути вилучення зображень |
| `--no-detect-tables` | — | Вимкнути виявлення таблиць |
| `--validate` | викл | Перевірка якості вихідного Markdown |
| `--resume` | — | Відновити перервану обробку |

### `reprocess` — Повторна обробка невдалих сторінок

```bash
# Автовизначення і повторна обробка з файлу стану
superbook-pdf reprocess output/.superbook-state.json

# Тільки певні сторінки
superbook-pdf reprocess output/.superbook-state.json -p 5,12,30

# Тільки перегляд статусу
superbook-pdf reprocess output/.superbook-state.json --status
```

---

## Встановлення

### Вимоги

| Компонент | Вимога |
|-----------|--------|
| ОС | Linux / macOS / Windows |
| Rust | 1.82+ (для збірки з вихідного коду) |
| Poppler | команда `pdftoppm` |

Для AI-функцій потрібні Python 3.10+ та GPU NVIDIA (CUDA 11.8+).

### 1. Системні залежності

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

### 2. Встановлення superbook-pdf

```bash
git clone https://github.com/clearclown/Rust_DN_SuperBook_PDF_Converter.git
cd Rust_DN_SuperBook_PDF_Converter/superbook-pdf
cargo build --release --features web
```

### 3. Запуск через Docker/Podman (рекомендується)

```bash
# GPU NVIDIA
docker compose up -d

# GPU AMD (ROCm)
docker compose -f docker-compose.yml -f docker-compose.rocm.yml up -d

# Тільки CPU
docker compose -f docker-compose.yml -f docker-compose.cpu.yml up -d
```

Відкрийте http://localhost:8080 у браузері.

---

## Веб-інтерфейс

![Веб-інтерфейс](../doc_img/webUI.png)

Браузерний інтерфейс із перетягуванням файлів для початку конвертації. Підтримка відображення прогресу в реальному часі через WebSocket.

```bash
# Рекомендується: фронтенд (Nginx) + бекенд (Rust API/WS)
docker compose up -d

# Прямий режим: тільки сервер API/WS
superbook-pdf serve --port 8080 --bind 0.0.0.0
```

---

## Документація

| Документ | Зміст |
|----------|-------|
| [docs/pipeline.md](../pipeline.md) | Детальний дизайн конвеєра обробки |
| [docs/commands.md](../commands.md) | Повний довідник команд та параметрів |
| [docs/configuration.md](../configuration.md) | Налаштування через конфігураційні файли (TOML) |
| [docs/docker.md](../docker.md) | Докладне керівництво Docker/Podman |
| [docs/development.md](../development.md) | Керівництво розробника (збірка, тести, архітектура) |

---

## Усунення несправностей

| Проблема | Рішення |
|----------|----------|
| `pdftoppm: command not found` | `sudo apt install poppler-utils` |
| RealESRGAN не працює | Перевірте AI-сервіси: `docker compose ps` та `superbook-pdf info` |
| GPU не використовується | Перевірте `docker compose ps` та `nvidia-smi`; при потребі використайте CPU (`-f docker-compose.cpu.yml`) |
| Нестача пам'яті | Використайте `--max-pages 10` або `--chunk-size 5` |
| Deskew спотворює зображення | Вимкніть з `--no-deskew` |
| Поля обрізають текст | Збільште буфер: `--margin-safety 1.0` |

---

## Ліцензія

AGPL v3.0 — [LICENSE](../../LICENSE)

---

## Подяки

- **Daiyuu Nobori** — оригінальна реалізація
- **[RealESRGAN](https://github.com/xinntao/Real-ESRGAN)** — AI-суперроздільність
- **[YomiToku](https://github.com/kotaro-kinoshita/yomitoku)** — японське OCR
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
