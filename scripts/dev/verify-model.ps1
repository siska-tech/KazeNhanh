#Requires -Version 5.1
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$ModelPath,
    [Parameter(Mandatory)][string]$TokenizerPath,
    [Parameter(Mandatory)][string]$ReferenceCasesPath,
    [switch]$Offline
)
. (Join-Path $PSScriptRoot 'common.ps1')
# Resolve relative asset paths before switching working directory.
$modelAsset = (Resolve-Path -LiteralPath $ModelPath).Path
$tokenizerAsset = (Resolve-Path -LiteralPath $TokenizerPath).Path
$referenceAsset = (Resolve-Path -LiteralPath $ReferenceCasesPath).Path
foreach ($asset in @($modelAsset, $tokenizerAsset, $referenceAsset)) {
    Write-Host "Asset: $asset SHA256=$((Get-FileHash -LiteralPath $asset -Algorithm SHA256).Hash)"
}
$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Push-Location $repoRoot
try {
    $runArgs = @('run', '--locked', '--features', 'legacy', '--example', 'inference_smoke')
    if ($Offline) { $runArgs += '--offline' }
    Invoke-KazeCargo -CargoArguments ($runArgs + @('--', $modelAsset, $tokenizerAsset, $referenceAsset))
} finally { Pop-Location }
