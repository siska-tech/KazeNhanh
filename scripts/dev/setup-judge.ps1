#Requires -Version 5.1
[CmdletBinding()]
param([switch]$Offline)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$manifest = Get-Content (Join-Path $repoRoot 'resources/models/qwen-judge.lock.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$assetDir = Join-Path $repoRoot 'target/qwen-judge'
New-Item -ItemType Directory -Force $assetDir | Out-Null
foreach ($file in $manifest.files) {
    $destination = Join-Path $assetDir $file.name
    if (-not (Test-Path -LiteralPath $destination)) {
        if ($Offline) { throw "Model asset missing: $destination. Run setup-judge.ps1 online once." }
        $partial = "$destination.part"
        Write-Host "Downloading $($file.name)..."
        $previousProgress = $ProgressPreference
        try {
            $ProgressPreference = 'SilentlyContinue'
            Invoke-WebRequest -Uri $file.url -OutFile $partial -UseBasicParsing -TimeoutSec 300
        } finally { $ProgressPreference = $previousProgress }
        if ((Get-FileHash -LiteralPath $partial -Algorithm SHA256).Hash -ne $file.sha256) {
            throw "Model asset SHA256 mismatch: $partial. Asset was not installed."
        }
        Move-Item -LiteralPath $partial -Destination $destination -Force
    }
    if ((Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash -ne $file.sha256) {
        throw "Cached model asset SHA256 mismatch: $destination. Remove this file and retry."
    }
    Write-Host "Verified $($file.name): $($file.sha256)"
}
Write-Host "Model ready: $assetDir ($($manifest.license))"
