#Requires -Version 5.1
[CmdletBinding()]
param([switch]$Offline)
. (Join-Path $PSScriptRoot 'common.ps1')
$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Push-Location $repoRoot
try {
    ./scripts/dev/setup-judge.ps1 -Offline:$Offline
    $cargoArgs = @('run', '--release', '--locked', '--features', 'qwen', '--example', 'judge_smoke')
    if ($Offline) { $cargoArgs += '--offline' }
    $cargoArgs += @('--', 'target/judge-smoke-release.json')
    Invoke-KazeCargo -CargoArguments $cargoArgs
} finally { Pop-Location }