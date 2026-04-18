#!/usr/bin/env python3
"""
YomiToku Bridge for superbook-pdf

Japanese AI-OCR using YomiToku for high-accuracy text recognition.
Backend implementation used by FastAPI HTTP service (app.py).

Usage:
    python yomitoku_bridge.py INPUT [options]

Options:
    --output        Output directory or file
    --format        Output format: json, text, pdf, markdown (default: json)
    --gpu           GPU device ID (default: 0)
    --no-gpu        Disable GPU, use CPU only
    --confidence    Minimum confidence threshold (0.0-1.0, default: 0.5)

Exit codes:
    0: Success
    1: General error
    2: Invalid arguments
    3: Input not found
    4: Output error
    5: GPU/CUDA error
    6: Out of memory
"""

import argparse
import json
import os
import sys
import time
from pathlib import Path
from typing import List, Dict, Any, Optional

try:
    import torch
except ImportError:
    torch = None

try:
    from yomitoku import DocumentAnalyzer
    from yomitoku.data.functions import load_image
    YOMITOKU_AVAILABLE = True
except ImportError:
    YOMITOKU_AVAILABLE = False


# Exit codes matching Rust exit_codes
EXIT_SUCCESS = 0
EXIT_ERROR = 1
EXIT_INVALID_ARGS = 2
EXIT_INPUT_NOT_FOUND = 3
EXIT_OUTPUT_ERROR = 4
EXIT_GPU_ERROR = 5
EXIT_OOM = 6


def detect_text_direction(blocks: List[Dict]) -> str:
    """Detect overall text direction from blocks."""
    if not blocks:
        return "horizontal"

    vertical_count = 0
    horizontal_count = 0

    for block in blocks:
        direction = block.get("direction", "horizontal")
        if direction == "vertical":
            vertical_count += 1
        else:
            horizontal_count += 1

    if vertical_count > horizontal_count * 2:
        return "vertical"
    elif horizontal_count > vertical_count * 2:
        return "horizontal"
    else:
        return "mixed"


def _sort_blocks_reading_order(blocks: List[Dict]) -> List[Dict]:
    """Sort OCR blocks in natural reading order.

    - horizontal: top-to-bottom, then left-to-right
    - vertical: right-to-left columns, then top-to-bottom
    - mixed: fallback to horizontal-like ordering
    """
    if not blocks:
        return blocks

    direction = detect_text_direction(blocks)

    def center_xy(b: Dict[str, Any]) -> tuple:
        bb = b.get("bbox", [0.0, 0.0, 0.0, 0.0])
        try:
            x1, y1, x2, y2 = float(bb[0]), float(bb[1]), float(bb[2]), float(bb[3])
            return ((x1 + x2) * 0.5, (y1 + y2) * 0.5)
        except (TypeError, ValueError, IndexError):
            return (0.0, 0.0)

    if direction == "vertical":
        # Prefer vertical blocks first. Then right-to-left columns, top-to-bottom.
        return sorted(
            blocks,
            key=lambda b: (
                0 if b.get("direction") == "vertical" else 1,
                -round(center_xy(b)[0] / 24.0),
                round(center_xy(b)[1] / 24.0),
            ),
        )

    # horizontal or mixed fallback: prefer horizontal blocks first.
    return sorted(
        blocks,
        key=lambda b: (
            0 if b.get("direction") == "horizontal" else 1,
            round(center_xy(b)[1] / 24.0),
            round(center_xy(b)[0] / 24.0),
        ),
    )


def _should_keep_word(text: str, confidence: float, confidence_threshold: float) -> bool:
    """Keep words using threshold with salvage for long low-confidence lines."""
    if confidence >= confidence_threshold:
        return True

    # Many book lines are long but scored slightly low; keep them if score is not too low.
    compact = "".join(text.split())
    if len(compact) >= 18 and confidence >= 0.30:
        return True

    return False


def _normalize_bbox(box_like: Any) -> List[float]:
    """Normalize various bbox representations to [x1, y1, x2, y2]."""
    if box_like is None:
        return [0.0, 0.0, 0.0, 0.0]

    # points=[[x,y], ...] or [[x,y], ...]
    points = getattr(box_like, "points", None)
    if points is None and isinstance(box_like, (list, tuple)) and box_like and isinstance(box_like[0], (list, tuple)):
        points = box_like
    if points is not None:
        xs = []
        ys = []
        for p in points:
            if isinstance(p, (list, tuple)) and len(p) >= 2:
                try:
                    xs.append(float(p[0]))
                    ys.append(float(p[1]))
                except (TypeError, ValueError):
                    continue
        if xs and ys:
            return [min(xs), min(ys), max(xs), max(ys)]

    # Object-style x/y/w/h or left/top/right/bottom
    for attrs in (("x", "y", "w", "h"), ("left", "top", "right", "bottom")):
        if all(hasattr(box_like, a) for a in attrs):
            try:
                a0 = float(getattr(box_like, attrs[0]))
                a1 = float(getattr(box_like, attrs[1]))
                a2 = float(getattr(box_like, attrs[2]))
                a3 = float(getattr(box_like, attrs[3]))
                if attrs == ("x", "y", "w", "h"):
                    return [a0, a1, a0 + a2, a1 + a3]
                return [a0, a1, a2, a3]
            except (TypeError, ValueError):
                break

    # Dict-style boxes
    if isinstance(box_like, dict):
        keys = box_like.keys()
        if all(k in keys for k in ("x", "y", "w", "h")):
            try:
                x = float(box_like["x"])
                y = float(box_like["y"])
                w = float(box_like["w"])
                h = float(box_like["h"])
                return [x, y, x + w, y + h]
            except (TypeError, ValueError):
                pass
        if all(k in keys for k in ("left", "top", "right", "bottom")):
            try:
                return [
                    float(box_like["left"]),
                    float(box_like["top"]),
                    float(box_like["right"]),
                    float(box_like["bottom"]),
                ]
            except (TypeError, ValueError):
                pass

    # Flat list/tuple: [x1,y1,x2,y2] or [x,y,w,h]
    if isinstance(box_like, (list, tuple)) and len(box_like) >= 4:
        try:
            x0 = float(box_like[0])
            y0 = float(box_like[1])
            x2 = float(box_like[2])
            y2 = float(box_like[3])
            if x2 <= x0 or y2 <= y0:
                # Assume [x, y, w, h]
                return [x0, y0, x0 + x2, y0 + y2]
            return [x0, y0, x2, y2]
        except (TypeError, ValueError):
            return [0.0, 0.0, 0.0, 0.0]

    return [0.0, 0.0, 0.0, 0.0]


def process_image(
    input_path: Path,
    output_format: str = "json",
    gpu_id: Optional[int] = 0,
    confidence_threshold: float = 0.5,
) -> Dict[str, Any]:
    """Process a single image with YomiToku OCR."""
    start_time = time.time()

    if not input_path.exists():
        return {"error": "Input not found", "exit_code": EXIT_INPUT_NOT_FOUND}

    if not YOMITOKU_AVAILABLE:
        return {
            "error": "YomiToku not installed. Install with: pip install yomitoku",
            "exit_code": EXIT_ERROR,
        }

    try:
        # Configure device
        if gpu_id is not None and torch is not None and torch.cuda.is_available():
            device = f"cuda:{gpu_id}"
        else:
            device = "cpu"

        # Initialize analyzer
        analyzer = DocumentAnalyzer(device=device)

        # Load and process image
        # load_image returns a list of numpy arrays (for batch processing)
        img_list = load_image(str(input_path))
        if isinstance(img_list, list) and len(img_list) > 0:
            img = img_list[0]
        else:
            img = img_list
        result_tuple = analyzer(img)
        # YomiToku 0.10+ returns a tuple (DocumentAnalyzerSchema, None, None)
#        result = result_tuple[0] if isinstance(result_tuple, tuple) else result_tuple
#
#        # Extract text blocks from paragraphs
#        text_blocks = []
#        full_text = []
#
#        # Process paragraphs (main text blocks)
#        for para in getattr(result, "paragraphs", []):
#            # Parse box - can be [x1, y1, x2, y2] or a Box object
#            box = para.box if hasattr(para, "box") else getattr(para, "bbox", [0, 0, 0, 0])
#            if hasattr(box, "__iter__") and not isinstance(box, str):
#                box_list = list(box)
#            else:
#                box_list = [0, 0, 0, 0]
#
#            # Get text content
#            text = para.contents if hasattr(para, "contents") else str(para)
#            
#            # Get direction
#            direction = getattr(para, "direction", "horizontal")
#            
#            block = {
#                "text": text,
#                "bbox": box_list,
#                "confidence": 1.0,  # YomiToku doesn't provide per-block confidence
#                "direction": direction,
#            }
#            text_blocks.append(block)
#            full_text.append(text)

        # YomiToku 0.10+ returns (DocumentAnalyzerSchema, None, None).
        if isinstance(result_tuple, tuple):
            result = result_tuple[0] if len(result_tuple) > 0 else None
        else:
            result = result_tuple

        text_blocks = []
        full_text = []

        # 1) Word-level extraction first to maximize OCR coverage for searchable PDF.
        for word in getattr(result, "words", []):
            text = getattr(word, "content", "")
            if not text:
                continue

            confidence = float(getattr(word, "rec_score", 1.0))
            if not _should_keep_word(text, confidence, confidence_threshold):
                continue

            box = getattr(word, "box", getattr(word, "points", None))
            direction = getattr(word, "direction", "horizontal")
            text_blocks.append({
                "text": text,
                "bbox": _normalize_bbox(box),
                "confidence": confidence,
                "direction": direction,
            })
            full_text.append(text)

        # 2) Fallback: paragraph-level extraction if words are unavailable.
        if not text_blocks:
            for para in getattr(result, "paragraphs", []):
                text = getattr(para, "contents", "")
                if not text:
                    continue

                box = getattr(para, "box", getattr(para, "bbox", None))
                direction = getattr(para, "direction", "horizontal")
                text_blocks.append({
                    "text": text,
                    "bbox": _normalize_bbox(box),
                    "confidence": 1.0,
                    "direction": direction,
                })
                full_text.append(text)

        # Normalize reading order before exporting to Rust side.
        text_blocks = _sort_blocks_reading_order(text_blocks)
        full_text = [b.get("text", "") for b in text_blocks if b.get("text", "")]

        elapsed = time.time() - start_time

        # Calculate overall confidence (set to 1.0 since YomiToku doesn't provide it)
        avg_confidence = 1.0 if text_blocks else 0.0

        return {
            "input_path": str(input_path),
            "text_blocks": text_blocks,
            "full_text": "\n".join(full_text),
            "confidence": avg_confidence,
            "text_direction": detect_text_direction(text_blocks),
            "processing_time": elapsed,
            "exit_code": EXIT_SUCCESS,
        }

    except RuntimeError as e:
        error_str = str(e).lower()
        if "cuda" in error_str or "gpu" in error_str:
            return {"error": str(e), "exit_code": EXIT_GPU_ERROR}
        elif "out of memory" in error_str:
            return {"error": "Out of memory", "exit_code": EXIT_OOM}
        return {"error": str(e), "exit_code": EXIT_ERROR}
    except Exception as e:
        return {"error": str(e), "exit_code": EXIT_ERROR}


def format_output(result: Dict[str, Any], output_format: str) -> str:
    """Format result according to specified output format."""
    if output_format == "json":
        return json.dumps(result, ensure_ascii=False, indent=2)
    elif output_format == "text":
        return result.get("full_text", "")
    elif output_format == "markdown":
        lines = [f"# OCR Result: {result.get('input_path', 'Unknown')}"]
        lines.append("")
        lines.append(f"**Confidence:** {result.get('confidence', 0):.2%}")
        lines.append(f"**Direction:** {result.get('text_direction', 'unknown')}")
        lines.append(f"**Processing Time:** {result.get('processing_time', 0):.2f}s")
        lines.append("")
        lines.append("## Detected Text")
        lines.append("")
        lines.append(result.get("full_text", ""))
        return "\n".join(lines)
    else:
        return json.dumps(result, ensure_ascii=False)


def main():
    parser = argparse.ArgumentParser(description="YomiToku Japanese AI-OCR")
    parser.add_argument("input", help="Input image or directory")
    parser.add_argument("--output", "-o", help="Output path")
    parser.add_argument(
        "--format",
        "-f",
        default="json",
        choices=["json", "text", "pdf", "markdown"],
        help="Output format",
    )
    parser.add_argument("--gpu", "-g", type=int, default=0, help="GPU device ID")
    parser.add_argument("--no-gpu", action="store_true", help="Disable GPU")
    parser.add_argument(
        "--confidence",
        "-c",
        type=float,
        default=0.5,
        help="Confidence threshold (0.0-1.0)",
    )

    args = parser.parse_args()

    input_path = Path(args.input)
    gpu_id = None if args.no_gpu else args.gpu

    if not input_path.exists():
        error_result = {"error": "Input not found", "exit_code": EXIT_INPUT_NOT_FOUND}
        print(json.dumps(error_result), file=sys.stderr)
        sys.exit(EXIT_INPUT_NOT_FOUND)

    # Process single file
    if input_path.is_file():
        result = process_image(
            input_path,
            output_format=args.format,
            gpu_id=gpu_id,
            confidence_threshold=args.confidence,
        )

        output = format_output(result, args.format)

        if args.output:
            output_path = Path(args.output)
            output_path.parent.mkdir(parents=True, exist_ok=True)
            output_path.write_text(output, encoding="utf-8")
            print(f"Output written to: {output_path}")
        else:
            print(output)

        sys.exit(result.get("exit_code", EXIT_ERROR))

    # Process directory
    elif input_path.is_dir():
        results = []
        image_extensions = {".png", ".jpg", ".jpeg", ".webp", ".bmp", ".tiff"}

        for img_file in sorted(input_path.iterdir()):
            if img_file.suffix.lower() in image_extensions:
                result = process_image(
                    img_file,
                    output_format=args.format,
                    gpu_id=gpu_id,
                    confidence_threshold=args.confidence,
                )
                results.append(result)

                # Print progress
                if result.get("exit_code", 1) == EXIT_SUCCESS:
                    block_count = len(result.get("text_blocks", []))
                    print(
                        f"Processed: {img_file.name} ({block_count} blocks, "
                        f"{result.get('processing_time', 0):.2f}s)",
                        file=sys.stderr,
                    )
                else:
                    print(
                        f"Failed: {img_file.name} - {result.get('error', 'Unknown')}",
                        file=sys.stderr,
                    )

        # Output batch results
        batch_result = {
            "results": results,
            "total": len(results),
            "successful": len([r for r in results if r.get("exit_code") == EXIT_SUCCESS]),
        }

        if args.output:
            output_path = Path(args.output)
            output_path.parent.mkdir(parents=True, exist_ok=True)
            output_path.write_text(
                json.dumps(batch_result, ensure_ascii=False, indent=2),
                encoding="utf-8",
            )
            print(f"Batch output written to: {output_path}")
        else:
            print(json.dumps(batch_result, ensure_ascii=False, indent=2))

        # Exit with error if any failed
        failed = [r for r in results if r.get("exit_code", 1) != EXIT_SUCCESS]
        if failed:
            sys.exit(EXIT_ERROR)

    else:
        error_result = {"error": "Invalid input path", "exit_code": EXIT_INVALID_ARGS}
        print(json.dumps(error_result), file=sys.stderr)
        sys.exit(EXIT_INVALID_ARGS)


if __name__ == "__main__":
    main()
