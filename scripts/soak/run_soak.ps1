#Requires -Version 5.1
Param(
    [ValidateRange(0, 1440)][int]$DurationMinutes = 30,
    [ValidateRange(1, 3600)][int]$IntervalSeconds = 120
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
. (Join-Path $projectRoot 'scripts/dev/common.ps1')
$previousOutput = $env:CRITERION_OUTPUT
$previousDebug = $env:CRITERION_DEBUG
Push-Location $projectRoot
try {
    $timestamp = Get-Date -Format 'yyyyMMdd-HHmmss'
    $artifactRoot = Join-Path $projectRoot "artifacts/soak/$timestamp"
    New-Item -ItemType Directory -Path $artifactRoot -Force | Out-Null
    Write-Host "[soak] Starting mock workflow run: ${DurationMinutes}m"
    $stopwatch = [Diagnostics.Stopwatch]::StartNew()
    $iteration = 0
    while ($stopwatch.Elapsed.TotalMinutes -lt $DurationMinutes) {
        $iteration++
        $benchLog = Join-Path $artifactRoot "bench-$iteration.log"
        $env:CRITERION_DEBUG = 'true'
        $env:CRITERION_OUTPUT = Join-Path $artifactRoot "criterion-$iteration"
        # Native stdout/stderr are logs; Criterion produces separate JSON artifacts.
        $launch = @{
            FilePath = Get-KazeCargo
            ArgumentList = @('bench', '--locked', '--features', 'mock_inference', '--bench', 'performance', '--', '--sample-size', '25', '--measurement-time', '3', '--warm-up-time', '1')
            WorkingDirectory = $projectRoot
            RedirectStandardOutput = $benchLog
            RedirectStandardError = "$benchLog.stderr"
            Wait = $true
            PassThru = $true
        }
        if ($env:OS -eq 'Windows_NT') { $launch.WindowStyle = 'Hidden' }
        $process = Start-Process @launch
        if ($process.ExitCode -ne 0) { throw "Soak benchmark failed (exit $($process.ExitCode)); see $benchLog and $benchLog.stderr" }
        $remainingSeconds = ($DurationMinutes * 60) - $stopwatch.Elapsed.TotalSeconds
        if ($remainingSeconds -gt 0) {
            Start-Sleep -Seconds ([int][Math]::Min($IntervalSeconds, [Math]::Ceiling($remainingSeconds)))
        }
    }
    $stopwatch.Stop()
    Write-Host "[soak] Finished. Artifacts: $artifactRoot"
} finally {
    $env:CRITERION_OUTPUT = $previousOutput
    $env:CRITERION_DEBUG = $previousDebug
    Pop-Location
}
