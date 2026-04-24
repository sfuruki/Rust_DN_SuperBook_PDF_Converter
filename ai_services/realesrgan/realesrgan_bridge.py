#!/usr/bin/env python3
"""
RealESRGAN Bridge for superbook-pdf

Upscale images using Real-ESRGAN for AI-enhanced super-resolution.
Backend implementation used by FastAPI HTTP service (app.py).

Usage:
    python realesrgan_bridge.py -i INPUT -o OUTPUT [options]

Options:
    -i, --input     Input image path or directory
    -o, --output    Output path (file or directory)
    -s, --scale     Upscale factor (2 or 4, default: 2)
    -t, --tile      Tile size for processing (default: 400)
    -g, --gpu       GPU device ID (default: 0)
    --model         Model name (default: realesrgan-x4plus)
    --fp32          Use FP32 precision instead of FP16
    --json          Output result as JSON

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
import threading
from pathlib import Path

try:
    import torch
    from basicsr.archs.rrdbnet_arch import RRDBNet
    from realesrgan import RealESRGANer
    import cv2
    import numpy as np
except ImportError as e:
    print(f"Error: Required package not installed: {e}", file=sys.stderr)
    print("Install with: pip install realesrgan basicsr torch opencv-python", file=sys.stderr)
    sys.exit(2)


# Exit codes matching Rust exit_codes
EXIT_SUCCESS = 0
EXIT_ERROR = 1
EXIT_INVALID_ARGS = 2
EXIT_INPUT_NOT_FOUND = 3
EXIT_OUTPUT_ERROR = 4
EXIT_GPU_ERROR = 5
EXIT_OOM = 6


MODEL_URLS = {
    "RealESRGAN_x4plus.pth": "https://github.com/xinntao/Real-ESRGAN/releases/download/v0.1.0/RealESRGAN_x4plus.pth",
    "RealESRGAN_x2plus.pth": "https://github.com/xinntao/Real-ESRGAN/releases/download/v0.2.1/RealESRGAN_x2plus.pth",
    "RealESRGAN_x4plus_anime_6B.pth": "https://github.com/xinntao/Real-ESRGAN/releases/download/v0.2.2.4/RealESRGAN_x4plus_anime_6B.pth",
}

MODEL_MIN_BYTES = {
    "RealESRGAN_x4plus.pth": 60 * 1024 * 1024,
    "RealESRGAN_x2plus.pth": 30 * 1024 * 1024,
    "RealESRGAN_x4plus_anime_6B.pth": 15 * 1024 * 1024,
}


# Reuse loaded upsampler instances across requests to avoid repeated model init.
# Each cached instance has a dedicated lock because RealESRGANer is not re-entrant.
_UPSAMPLER_CACHE = {}
_UPSAMPLER_CACHE_LOCK = threading.Lock()
_ACTIVE_INFERENCE = 0
_ACTIVE_INFERENCE_LOCK = threading.Lock()
_ACTIVE_INFERENCE_COND = threading.Condition(_ACTIVE_INFERENCE_LOCK)
_MEASURED_INFERENCE_MB = 0.0
_MEASURED_INFERENCE_SAMPLES = 0
_MEASURED_INFERENCE_LOCK = threading.Lock()
_POOL_SIZE_ENV_RAW = os.environ.get("REALESRGAN_POOL_SIZE", "1")


def _parse_env_int(name: str, default: int) -> int:
    try:
        return int(os.environ.get(name, str(default)))
    except ValueError:
        return default


def _parse_env_float(name: str, default: float) -> float:
    try:
        return float(os.environ.get(name, str(default)))
    except ValueError:
        return default


_AUTO_POOL_MAX = _parse_env_int("REALESRGAN_POOL_MAX", 8)
_GPU_SAFETY_MARGIN_MB = _parse_env_float("GPU_SAFETY_MARGIN_MB", 3000.0)
_BOOTSTRAP_INFERENCE_MB = _parse_env_float("REALESRGAN_BOOTSTRAP_INFERENCE_MB", 0.0)


def _configured_pool_size() -> int:
    """Resolve configured pool size. 0 means dynamic runtime auto control."""
    raw = _POOL_SIZE_ENV_RAW
    try:
        configured = int(raw)
    except ValueError:
        configured = 1
    return max(0, configured)


_CONFIGURED_POOL_SIZE = _configured_pool_size()


def _current_per_inference_mb() -> float:
    with _MEASURED_INFERENCE_LOCK:
        measured = _MEASURED_INFERENCE_MB
        samples = _MEASURED_INFERENCE_SAMPLES

    if samples > 0 and measured > 0.0:
        return max(256.0, measured)

    # Bootstrap fallback before measured samples are available.
    # The previous fixed 1200MB underestimated x4plus load and caused burst oversubscription.
    if _BOOTSTRAP_INFERENCE_MB > 0:
        return max(512.0, _BOOTSTRAP_INFERENCE_MB)

    if torch.cuda.is_available():
        try:
            total_mb = torch.cuda.get_device_properties(0).total_memory / 1024**2
            # Conservative bootstrap: around half of total VRAM on 12GB class GPUs.
            return max(4096.0, total_mb * 0.45)
        except Exception:
            pass

    return 4096.0


def _current_pool_limit(gpu_id: int = 0, active_override: int | None = None) -> int:
    # CRITICAL: RealESRGANer is NOT re-entrant. Multiple concurrent instances
    # on the same GPU cause internal buffer corruption → "illegal memory access" errors.
    # Force strict single-instance constraint. GPU-level concurrency is controlled
    # by _ACTIVE_INFERENCE semaphore (multiple requests wait sequentially).
    if _CONFIGURED_POOL_SIZE > 0:
        return max(1, _CONFIGURED_POOL_SIZE)

    # Always enforce single instance on GPU to prevent re-entrance corruption
    return 1


def _inc_active_inference():
    global _ACTIVE_INFERENCE
    with _ACTIVE_INFERENCE_LOCK:
        _ACTIVE_INFERENCE += 1


def _dec_active_inference():
    global _ACTIVE_INFERENCE
    with _ACTIVE_INFERENCE_LOCK:
        _ACTIVE_INFERENCE = max(0, _ACTIVE_INFERENCE - 1)


def _dec_active_inference_locked():
    global _ACTIVE_INFERENCE
    _ACTIVE_INFERENCE = max(0, _ACTIVE_INFERENCE - 1)


def _update_measured_inference_mb(sample_mb: float):
    """Update EWMA for measured per-inference VRAM usage."""
    global _MEASURED_INFERENCE_MB, _MEASURED_INFERENCE_SAMPLES
    if sample_mb <= 0:
        return

    with _MEASURED_INFERENCE_LOCK:
        if _MEASURED_INFERENCE_SAMPLES == 0:
            _MEASURED_INFERENCE_MB = sample_mb
        else:
            _MEASURED_INFERENCE_MB = (_MEASURED_INFERENCE_MB * 0.8) + (sample_mb * 0.2)
        _MEASURED_INFERENCE_SAMPLES += 1


def _begin_inference_measurement(gpu_id: int):
    """Begin inference under runtime dynamic cap and capture optional baseline."""
    global _ACTIVE_INFERENCE
    with _ACTIVE_INFERENCE_COND:
        while True:
            cap = _current_pool_limit(gpu_id, active_override=_ACTIVE_INFERENCE)
            if _ACTIVE_INFERENCE < cap:
                _ACTIVE_INFERENCE += 1
                active_now = _ACTIVE_INFERENCE
                break
            # Wait until another inference finishes and releases a slot.
            _ACTIVE_INFERENCE_COND.wait(timeout=0.05)

    baseline_reserved_mb = None
    if active_now == 1 and torch.cuda.is_available():
        try:
            torch.cuda.synchronize(gpu_id)
            baseline_reserved_mb = torch.cuda.memory_reserved(gpu_id) / 1024**2
        except Exception:
            baseline_reserved_mb = None

    return baseline_reserved_mb


def _end_inference_measurement(gpu_id: int, baseline_reserved_mb):
    """Finish inference and record measured VRAM delta when isolated."""
    try:
        if baseline_reserved_mb is not None and torch.cuda.is_available():
            try:
                torch.cuda.synchronize(gpu_id)
                end_reserved_mb = torch.cuda.memory_reserved(gpu_id) / 1024**2
                measured_delta_mb = max(0.0, end_reserved_mb - baseline_reserved_mb)
                _update_measured_inference_mb(measured_delta_mb)
            except Exception:
                pass
    finally:
        with _ACTIVE_INFERENCE_COND:
            _dec_active_inference_locked()
            _ACTIVE_INFERENCE_COND.notify_all()


def get_runtime_stats() -> dict:
    with _UPSAMPLER_CACHE_LOCK:
        slot_count = sum(len(v.get("entries", [])) for v in _UPSAMPLER_CACHE.values())
        cache_keys = len(_UPSAMPLER_CACHE)

    with _ACTIVE_INFERENCE_LOCK:
        active_inference = _ACTIVE_INFERENCE

    with _MEASURED_INFERENCE_LOCK:
        measured_inference_mb = _MEASURED_INFERENCE_MB
        measured_inference_samples = _MEASURED_INFERENCE_SAMPLES

    current_pool_limit = _current_pool_limit(0)

    return {
        "active_inference": active_inference,
        "upsampler_pool_size": current_pool_limit,
        "upsampler_slots": slot_count,
        "cache_keys": cache_keys,
        "measured_inference_mb": round(measured_inference_mb, 1),
        "measured_inference_samples": measured_inference_samples,
        "configured_pool_size": _CONFIGURED_POOL_SIZE,
    }


def _is_cuda_illegal_memory_access(err: BaseException) -> bool:
    return "illegal memory access" in str(err).lower()


def _is_valid_weight_file(path: Path, model_filename: str) -> bool:
    min_bytes = MODEL_MIN_BYTES.get(model_filename, 1)
    return path.exists() and path.stat().st_size >= min_bytes


def download_model(model_filename: str, target_path: Path) -> bool:
    """Download model weights if not present."""
    import urllib.request
    import ssl
    import shutil
    
    min_bytes = MODEL_MIN_BYTES.get(model_filename, 1)
    if target_path.exists() and target_path.stat().st_size >= min_bytes:
        return True
    if target_path.exists() and target_path.stat().st_size < min_bytes:
        print(f"Corrupt/incomplete model detected, re-downloading: {target_path}", file=sys.stderr)
        target_path.unlink(missing_ok=True)
    
    url = MODEL_URLS.get(model_filename)
    if not url:
        print(f"Unknown model: {model_filename}", file=sys.stderr)
        return False
    
    print(f"Downloading model: {model_filename}...", file=sys.stderr)
    target_path.parent.mkdir(parents=True, exist_ok=True)
    
    tmp_path = target_path.with_suffix(target_path.suffix + ".download")
    try:
        # Create SSL context that doesn't verify certificates (for GitHub redirects)
        ctx = ssl.create_default_context()
        ctx.check_hostname = False
        ctx.verify_mode = ssl.CERT_NONE

        with urllib.request.urlopen(url, context=ctx, timeout=60) as resp, open(tmp_path, "wb") as out_f:
            shutil.copyfileobj(resp, out_f)

        if tmp_path.stat().st_size < min_bytes:
            raise RuntimeError(
                f"Downloaded file too small: {tmp_path.stat().st_size} bytes (expected >= {min_bytes})"
            )

        tmp_path.replace(target_path)
        print(f"Downloaded: {target_path}", file=sys.stderr)
        return True
    except Exception as e:
        tmp_path.unlink(missing_ok=True)
        print(f"Download failed: {e}", file=sys.stderr)
        return False


def get_model(model_name: str, scale: int):
    """Load RealESRGAN model."""
    # Prefer persistent cache location so model files survive container recreation.
    script_dir = Path(__file__).parent
    cache_dir = Path.home() / ".cache" / "realesrgan"
    weights_dir = script_dir / "weights"
    
    if model_name == "realesrgan-x4plus":
        model = RRDBNet(
            num_in_ch=3,
            num_out_ch=3,
            num_feat=64,
            num_block=23,
            num_grow_ch=32,
            scale=4
        )
        netscale = 4
        model_filename = "RealESRGAN_x4plus.pth"
    elif model_name == "realesrgan-x2plus":
        model = RRDBNet(
            num_in_ch=3,
            num_out_ch=3,
            num_feat=64,
            num_block=23,
            num_grow_ch=32,
            scale=2
        )
        netscale = 2
        model_filename = "RealESRGAN_x2plus.pth"
    elif model_name == "realesrgan-x4plus-anime":
        model = RRDBNet(
            num_in_ch=3,
            num_out_ch=3,
            num_feat=64,
            num_block=6,
            num_grow_ch=32,
            scale=4
        )
        netscale = 4
        model_filename = "RealESRGAN_x4plus_anime_6B.pth"
    else:
        raise ValueError(f"Unknown model: {model_name}")
    
    model_path_candidates = [
        cache_dir / model_filename,
        weights_dir / model_filename,
        Path("/usr/share/realesrgan") / model_filename,
    ]

    model_path = None
    for candidate in model_path_candidates:
        if _is_valid_weight_file(candidate, model_filename):
            model_path = candidate
            break

        if candidate.exists():
            print(f"Corrupt/incomplete model detected, removing: {candidate}", file=sys.stderr)
            candidate.unlink(missing_ok=True)

    if model_path is None:
        target_path = cache_dir / model_filename
        if not download_model(model_filename, target_path):
            raise FileNotFoundError(f"Model weights not found and download failed: {model_filename}")
        model_path = target_path
    
    return model, netscale, str(model_path)


def get_or_create_upsampler(
    model_name: str,
    scale: int,
    tile: int,
    gpu_id: int,
    fp32: bool,
):
    """Get a cached RealESRGANer instance from a small round-robin pool."""
    cache_key = (model_name, scale, tile, gpu_id, fp32)
    with _UPSAMPLER_CACHE_LOCK:
        cached = _UPSAMPLER_CACHE.get(cache_key)
        if cached is not None:
            entries = cached["entries"]
            pool_limit = _current_pool_limit(gpu_id)
            if len(entries) < pool_limit:
                model, netscale, model_path = get_model(model_name, scale)
                upsampler = RealESRGANer(
                    scale=netscale,
                    model_path=model_path,
                    model=model,
                    tile=tile,
                    tile_pad=10,
                    pre_pad=0,
                    half=not fp32,
                    device=f"cuda:{gpu_id}" if torch.cuda.is_available() else "cpu",
                )
                entries.append((upsampler, threading.Lock()))
                print(
                    f"MODEL_CACHE expand: {cache_key} -> size={len(entries)} limit={pool_limit}",
                    file=sys.stderr,
                )

            idx = cached["next_idx"] % len(entries)
            cached["next_idx"] = (idx + 1) % len(entries)
            print(f"MODEL_CACHE hit: {cache_key} slot={idx}", file=sys.stderr)
            return entries[idx]

        print(f"MODEL_CACHE miss: {cache_key}", file=sys.stderr)
        model, netscale, model_path = get_model(model_name, scale)
        upsampler = RealESRGANer(
            scale=netscale,
            model_path=model_path,
            model=model,
            tile=tile,
            tile_pad=10,
            pre_pad=0,
            half=not fp32,
            device=f"cuda:{gpu_id}" if torch.cuda.is_available() else "cpu",
        )
        cache_entry = {
            "entries": [(upsampler, threading.Lock())],
            "next_idx": 0,
        }
        _UPSAMPLER_CACHE[cache_key] = cache_entry
        return cache_entry["entries"][0]


def upscale_image(
    input_path: Path,
    output_path: Path,
    scale: int = 2,
    tile: int = 400,
    gpu_id: int = 0,
    model_name: str = "realesrgan-x4plus",
    fp32: bool = False,
) -> dict:
    """Upscale a single image."""
    start_time = time.time()

    # Calculate output scale (model scale may differ from requested scale)
    outscale = scale

    cache_key = (model_name, scale, tile, gpu_id, fp32)

    try:
        try:
            upsampler, upsampler_lock = get_or_create_upsampler(
                model_name=model_name,
                scale=scale,
                tile=tile,
                gpu_id=gpu_id,
                fp32=fp32,
            )
        except RuntimeError as e:
            if "CUDA" in str(e) or "GPU" in str(e):
                return {"error": str(e), "exit_code": EXIT_GPU_ERROR}
            raise

        # Read image
        img = cv2.imread(str(input_path), cv2.IMREAD_UNCHANGED)
        if img is None:
            return {"error": f"Failed to read image: {input_path}", "exit_code": EXIT_INPUT_NOT_FOUND}

        original_size = img.shape[:2]

        output = None
        for attempt in range(2):
            try:
                # RealESRGANer keeps mutable buffers internally, so parallel calls to
                # the same cached instance can corrupt outputs. Guard each instance.
                with upsampler_lock:
                    baseline_reserved_mb = _begin_inference_measurement(gpu_id)
                    try:
                        output, _ = upsampler.enhance(img, outscale=outscale)
                    finally:
                        _end_inference_measurement(gpu_id, baseline_reserved_mb)
                break
            except RuntimeError as e:
                if "out of memory" in str(e).lower():
                    return {"error": "Out of memory", "exit_code": EXIT_OOM}

                # CUDA illegal memory access can poison current model/context.
                # Drop cache, clear allocator if possible, and retry once.
                if _is_cuda_illegal_memory_access(e) and attempt == 0:
                    print(
                        f"GPU fault detected. Reinitializing upsampler cache for {cache_key}",
                        file=sys.stderr,
                    )
                    with _UPSAMPLER_CACHE_LOCK:
                        _UPSAMPLER_CACHE.pop(cache_key, None)
                    if torch.cuda.is_available():
                        try:
                            torch.cuda.empty_cache()
                        except RuntimeError as clear_err:
                            print(
                                f"WARN: empty_cache failed during recovery: {clear_err}",
                                file=sys.stderr,
                            )

                    upsampler, upsampler_lock = get_or_create_upsampler(
                        model_name=model_name,
                        scale=scale,
                        tile=tile,
                        gpu_id=gpu_id,
                        fp32=fp32,
                    )
                    continue

                raise

        if output is None:
            return {
                "error": "Upscale failed without output",
                "exit_code": EXIT_ERROR,
            }

        # Save output
        output_path.parent.mkdir(parents=True, exist_ok=True)
        cv2.imwrite(str(output_path), output)

        elapsed = time.time() - start_time
        output_size = output.shape[:2]

        return {
            "input_path": str(input_path),
            "output_path": str(output_path),
            "original_size": list(original_size),
            "output_size": list(output_size),
            "scale": scale,
            "model": model_name,
            "processing_time": elapsed,
            "exit_code": EXIT_SUCCESS,
        }
    finally:
        # 推論後に一時確保を解放する（構築方针 5.2）
        # モデル自体（_UPSAMPLER_CACHE）は維持する
        if torch.cuda.is_available():
            try:
                torch.cuda.empty_cache()
            except RuntimeError as e:
                print(f"WARN: empty_cache failed: {e}", file=sys.stderr)


def main():
    parser = argparse.ArgumentParser(description="RealESRGAN image upscaler")
    parser.add_argument("-i", "--input", required=True, help="Input image or directory")
    parser.add_argument("-o", "--output", required=True, help="Output path")
    parser.add_argument("-s", "--scale", type=int, default=2, choices=[2, 4], help="Upscale factor")
    parser.add_argument("-t", "--tile", type=int, default=400, help="Tile size")
    parser.add_argument("-g", "--gpu", type=int, default=0, help="GPU device ID")
    parser.add_argument("--model", default="realesrgan-x4plus", help="Model name")
    parser.add_argument("--fp32", action="store_true", help="Use FP32 precision")
    parser.add_argument("--json", action="store_true", help="Output as JSON")

    args = parser.parse_args()

    input_path = Path(args.input)
    output_path = Path(args.output)

    if not input_path.exists():
        if args.json:
            print(json.dumps({"error": "Input not found", "exit_code": EXIT_INPUT_NOT_FOUND}))
        else:
            print(f"Error: Input not found: {input_path}", file=sys.stderr)
        sys.exit(EXIT_INPUT_NOT_FOUND)

    # Process single file or directory
    if input_path.is_file():
        result = upscale_image(
            input_path,
            output_path,
            scale=args.scale,
            tile=args.tile,
            gpu_id=args.gpu,
            model_name=args.model,
            fp32=args.fp32,
        )

        if args.json:
            print(json.dumps(result))
        elif result.get("exit_code", 1) == EXIT_SUCCESS:
            print(f"Upscaled: {input_path} -> {output_path} ({result['processing_time']:.2f}s)")
        else:
            print(f"Error: {result.get('error', 'Unknown error')}", file=sys.stderr)

        sys.exit(result.get("exit_code", EXIT_ERROR))

    elif input_path.is_dir():
        # Process directory
        results = []
        image_extensions = {".png", ".jpg", ".jpeg", ".webp", ".bmp", ".tiff"}

        output_path.mkdir(parents=True, exist_ok=True)

        for img_file in sorted(input_path.iterdir()):
            if img_file.suffix.lower() in image_extensions:
                out_file = output_path / f"{img_file.stem}{args.scale}x.png"
                result = upscale_image(
                    img_file,
                    out_file,
                    scale=args.scale,
                    tile=args.tile,
                    gpu_id=args.gpu,
                    model_name=args.model,
                    fp32=args.fp32,
                )
                results.append(result)

                if not args.json:
                    if result.get("exit_code", 1) == EXIT_SUCCESS:
                        print(f"Upscaled: {img_file.name} ({result['processing_time']:.2f}s)")
                    else:
                        print(f"Failed: {img_file.name} - {result.get('error', 'Unknown error')}", file=sys.stderr)

        if args.json:
            print(json.dumps({"results": results, "total": len(results)}))

        # Exit with error if any failed
        failed = [r for r in results if r.get("exit_code", 1) != EXIT_SUCCESS]
        if failed:
            sys.exit(EXIT_ERROR)

    else:
        if args.json:
            print(json.dumps({"error": "Invalid input path", "exit_code": EXIT_INVALID_ARGS}))
        else:
            print(f"Error: Invalid input path: {input_path}", file=sys.stderr)
        sys.exit(EXIT_INVALID_ARGS)


if __name__ == "__main__":
    main()
