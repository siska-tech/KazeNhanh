#Requires -Version 5.1
[CmdletBinding()]
param([switch]$Offline)
. (Join-Path $PSScriptRoot 'common.ps1')
$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Push-Location $repoRoot
try {
    $networkArgs = @()
    if ($Offline) { $networkArgs += '--offline' }
    $outputRoot = 'target/statistics-verification'
    New-Item -ItemType Directory -Force -Path $outputRoot | Out-Null
    $assetPath = Join-Path $outputRoot 'contract.json'
    $againPath = Join-Path $outputRoot 'contract-again.json'
    foreach ($path in @($assetPath, $againPath)) {
        Invoke-KazeCargo -CargoArguments (@('run', '--locked', '--example', 'statistics_asset') + $networkArgs + @('--',
            'tests/fixtures/statistics/clean-contract.jsonl', 'authored-contract-v1', 'contract_fixture', 'CC0-1.0', $path))
    }
    $assetHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $assetPath).Hash.ToLowerInvariant()
    if ($assetHash -ne (Get-FileHash -Algorithm SHA256 -LiteralPath $againPath).Hash.ToLowerInvariant()) {
        throw 'Statistics asset generation is not reproducible.'
    }
    $posAsset = Join-Path $outputRoot 'contract-pos.json'
    $posAgain = Join-Path $outputRoot 'contract-pos-again.json'
    foreach ($path in @($posAsset,$posAgain)) {
        Invoke-KazeCargo -CargoArguments (@('run','--locked','--example','statistics_asset') + $networkArgs + @('--',
            'tests/fixtures/statistics/clean-contract.jsonl','authored-contract-v1','contract_fixture','CC0-1.0',$path,'--with-pos'))
    }
    if ((Get-FileHash -Algorithm SHA256 -LiteralPath $posAsset).Hash -ne (Get-FileHash -Algorithm SHA256 -LiteralPath $posAgain).Hash) {
        throw 'POS asset generation is not reproducible.'
    }
    $pos = Get-Content -LiteralPath $posAsset -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($pos.schema_version -ne 'kzn.statistics.v2' -or $pos.pos.pair_count -le 0 -or $pos.pos.missing_pair_count -ne 0) { throw 'Expected complete Sudachi POS pairs.' }
    $summaries = @()
    # User-derived reports remain local: CI verifies synthetic contracts only.
    if ($env:CI -ne 'true') {
        foreach ($suffix in @('001', '002', '003')) {
            $inputPath = "evaluation/ppocrv6-medium-user-$suffix.jsonl"
            $reportPath = Join-Path $outputRoot "ocr-$suffix.json"
            Invoke-KazeCargo -CargoArguments (@('run', '--locked', '--example', 'recognition_samples') + $networkArgs + @('--',
                $inputPath, $reportPath, $assetPath, $assetHash, 'contract_fixture'))
            $report = Get-Content -Raw -Encoding UTF8 -LiteralPath $reportPath | ConvertFrom-Json
            foreach ($row in $report.reports) {
                $statistics = @($row.evidence | Where-Object kind -eq 'lexical_statistics')
                if ($statistics.Count -ne 1 -or $statistics[0].status -ne 'observed' -or $row.recognition_risk.value -ne $null -or
                    $row.metrics.slm_calls -ne 0 -or $row.decision -eq 'low_risk') {
                    throw 'Statistical observation must not claim risk probability or low risk.'
                }
            }
            $summaries += $report.summary
    }
    }
    # Hash rejection must occur before any report is written; use a fresh random target.
    $rejectPath = Join-Path $outputRoot (([guid]::NewGuid().ToString()) + '.json')
    $rejectArgs = @('run', '--locked', '--example', 'recognition_samples') + $networkArgs + @('--',
        'tests/fixtures/statistics/clean-contract.jsonl', $rejectPath, $assetPath, ('0' * 64), 'contract_fixture')
    & (Get-KazeCargo) @rejectArgs
    if ($LASTEXITCODE -eq 0 -or (Test-Path -LiteralPath $rejectPath)) { throw 'Mismatched statistics hash was accepted.' }
    $result = [ordered]@{
        schema_version = 'kzn.statistics.verification.v1'
        asset_sha256 = $assetHash
        corpus_role = 'synthetic_contract_fixture_only'
        quality_accepted = $false
        summaries = $summaries
    }
    [IO.File]::WriteAllText((Join-Path $repoRoot "$outputRoot/summary.json"), ($result | ConvertTo-Json -Depth 8), (New-Object Text.UTF8Encoding($false)))
    Write-Host 'Statistics reproducibility and hash rejection verified; local OCR observations retained when CI is unset.'
} finally { Pop-Location }
