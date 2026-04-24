#!/usr/bin/env powershell
<#
GPU Parallel Processing Test
Measures processing time and GPU usage with max_parallel_pages_gpu=4
#>

param(
    [int]$TimeoutSeconds = 600,
    [int]$PollIntervalSeconds = 2
)

$ErrorActionPreference = "Stop"

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "GPU Parallel Processing Test (max_parallel_pages_gpu=4)" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Check health
Write-Host "1. Service health check..." -ForegroundColor Yellow
$healthResp = curl.exe -sS http://localhost:8080/api/health 2>&1
if ($healthResp -notmatch '"status"') {
    Write-Host "ERROR: API health check failed" -ForegroundColor Red
    exit 1
}
Write-Host "   OK: API healthy" -ForegroundColor Green
Write-Host ""

# Submit conversion job with upscale enabled
Write-Host "2. Submit conversion job (test10p.pdf, upscale=true)" -ForegroundColor Yellow
$inline = @{
    upscale = @{
        enable = $true
        scale = 2
        model = "realesrgan-x4plus"
    }
    ocr = @{
        enable = $false
    }
    concurrency = @{
        max_parallel_pages_cpu = 0
        max_parallel_pages_gpu = 4
    }
} | ConvertTo-Json -Depth 5

$cfgFile = "data/work/test_parallel.json"
$inline | Out-File -FilePath $cfgFile -Encoding utf8

$resp = curl.exe -sS -X POST http://localhost:8080/api/convert `
    -F "file=@data/input/test10p.pdf" `
    -F "config_name=pipeline" `
    -F "inline_config=@$cfgFile;type=application/json" 2>&1

$jobJson = $resp | ConvertFrom-Json
$jobId = $jobJson.job_id
if (-not $jobId) {
    Write-Host "ERROR: job_id not found in response" -ForegroundColor Red
    Write-Host $resp
    exit 1
}
Write-Host "   Job ID: $jobId" -ForegroundColor Green
$startTime = Get-Date
Write-Host "   Start time: $($startTime.ToString('HH:mm:ss'))" -ForegroundColor Green
Write-Host ""

# Monitor progress and GPU stats
Write-Host "3. Monitoring job progress..." -ForegroundColor Yellow
Write-Host ""

$psCount = 0
$lastStep = ""
$gpuTimings = @()

$timeout = (Get-Date).AddSeconds($TimeoutSeconds)

while ((Get-Date) -lt $timeout) {
    # Get job status
    $job = curl.exe -sS http://localhost:8080/api/jobs/$jobId | ConvertFrom-Json
    
    if ($job.status -in @("completed", "failed", "cancelled")) {
        Write-Host ""
        Write-Host "   OK: Job finished - status=$($job.status)" -ForegroundColor Green
        break
    }
    
    $step = $job.progress.step_name
    $pct = $job.progress.percent
    
    if ($step -ne $lastStep) {
        $lastStep = $step
        Write-Host "   [$($psCount.ToString('D2'))] $step - $pct%"
    } else {
        Write-Host "   [$($psCount.ToString('D2'))] $step - $pct%" -ForegroundColor DarkGray
    }
    
    # Capture GPU stats every 5 polls
    if ($psCount % 5 -eq 0) {
        $stats = docker stats --no-stream --format "{{.CPUPerc}} {{.MemUsage}}" superbook-upscale 2>$null
        if ($stats) {
            $gpuTimings += @{
                elapsed = ((Get-Date) - $startTime).TotalSeconds
                stats = $stats
            }
            Write-Host "      [GPU] $stats" -ForegroundColor Cyan
        }
    }
    
    $psCount++
    Start-Sleep -Seconds $PollIntervalSeconds
}

$endTime = Get-Date
$elapsed = ($endTime - $startTime).TotalSeconds
$elapsedMin = [Math]::Round($elapsed / 60, 2)

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "Test Results" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Elapsed time: $elapsed sec ($elapsedMin min)" -ForegroundColor Green
Write-Host "Job status: $($job.status)" -ForegroundColor Green
Write-Host ""

# Check output
Write-Host "4. Output file check..." -ForegroundColor Yellow
$outputFile = "data/output/test10p_${jobId}_superbook.pdf"
if (Test-Path $outputFile) {
    $size = (Get-Item $outputFile).Length
    Write-Host "   OK: Output PDF: $outputFile" -ForegroundColor Green
    Write-Host "     File size: $([Math]::Round($size / 1MB, 2)) MB" -ForegroundColor Green
} else {
    Write-Host "   ERROR: Output file not found" -ForegroundColor Red
}
Write-Host ""

# GPU memory check
Write-Host "5. GPU memory status..." -ForegroundColor Yellow
$memStatus = curl.exe -sS http://localhost:8000/status 2>$null | ConvertFrom-Json
if ($memStatus.gpu) {
    Write-Host "   Memory usage:" -ForegroundColor Green
    Write-Host "     Allocated: $($memStatus.gpu.memory_allocated_mb) MB" -ForegroundColor Cyan
    Write-Host "     Reserved: $($memStatus.gpu.memory_reserved_mb) MB" -ForegroundColor Cyan
    Write-Host "     Total: $($memStatus.gpu.memory_total_mb) MB" -ForegroundColor Cyan
} else {
    Write-Host "   GPU status unavailable" -ForegroundColor Yellow
}
Write-Host ""

# GPU timing summary
if ($gpuTimings.Count -gt 0) {
    Write-Host "6. GPU utilization timeline..." -ForegroundColor Yellow
    $gpuTimings | ForEach-Object {
        $t = [Math]::Round($_.elapsed, 1)
        Write-Host "   @ ${t}s: $($_.stats)" -ForegroundColor Cyan
    }
    Write-Host ""
}

Write-Host "OK: Test completed" -ForegroundColor Green
