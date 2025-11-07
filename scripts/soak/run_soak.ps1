#Requires -Version 7.0

Param(
    [int]$DurationMinutes = 30,
    [int]$IntervalSeconds = 120
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$projectRoot = Split-Path -Parent $PSScriptRoot
Set-Location $projectRoot

$timestamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$artifactRoot = Join-Path $projectRoot "artifacts\soak\$timestamp"
New-Item -ItemType Directory -Path $artifactRoot -Force | Out-Null

Write-Host "[soak] Starting soak run. Duration=${DurationMinutes}m, Interval=${IntervalSeconds}s" -ForegroundColor Cyan

$stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
$iteration = 0

while ($stopwatch.Elapsed.TotalMinutes -lt $DurationMinutes) {
    $iteration++
    $iterationStamp = Get-Date -Format 'HH:mm:ss'
    Write-Host "[soak] Iteration #$iteration ($iterationStamp) running cargo bench..." -ForegroundColor Green

    $benchLog = Join-Path $artifactRoot "bench-$iteration.json"
    $env:CRITERION_DEBUG = 'true'
    $env:CRITERION_OUTPUT = $artifactRoot

    cargo bench --features mock_inference --bench performance -- --sample-size 25 --measurement-time 3 --warm-up-time 1 `
        *>$benchLog

    $env:CRITERION_DEBUG = $null
    $env:CRITERION_OUTPUT = $null

    Write-Host "[soak] Iteration #$iteration complete. Sleeping ${IntervalSeconds}s..." -ForegroundColor Yellow
    Start-Sleep -Seconds $IntervalSeconds
}

$stopwatch.Stop()

Write-Host "[soak] Soak run finished in $([int]$stopwatch.Elapsed.TotalMinutes)m. Artifacts: $artifactRoot" -ForegroundColor Cyan

