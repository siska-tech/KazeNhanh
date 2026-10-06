#Requires -Version 5.1
[CmdletBinding()]
param([switch]$Offline, [switch]$WithPos)
. (Join-Path $PSScriptRoot 'common.ps1')
$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Push-Location $repoRoot
try {
    if ($env:CI -eq 'true') { throw 'Dictionary matrix includes user-derived reports. Run locally; no CI upload is configured.' }
    $networkArgs = @()
    if ($Offline) { $networkArgs += '--offline' }
    $outputRoot = if ($WithPos) { 'target/dictionary-pos-matrix' } else { 'target/dictionary-matrix' }
    $statisticsFlags = @(); if ($WithPos) { $statisticsFlags += '--with-pos' }
    New-Item -ItemType Directory -Force -Path $outputRoot | Out-Null
    $allCases = @{}
    $editions = @()
    $hardCleanCases = @{}
    foreach ($edition in @('small', 'core', 'full')) {
        & (Join-Path $PSScriptRoot 'setup-sudachi.ps1') -Edition $edition -Offline:$Offline
        $dictionary = if ($edition -eq 'small') { 'resources/sudachi/system.dic' } else { "target/sudachi-dictionaries/$edition/system.dic" }
        $directory = Join-Path $outputRoot $edition
        New-Item -ItemType Directory -Force -Path $directory | Out-Null
        Invoke-KazeCargo -CargoArguments (@('run', '--locked', '--example', 'candidate_baseline') + $networkArgs + @('--',
            $dictionary, 'evaluation/candidate-review-contract.jsonl', (Join-Path $directory 'candidate-review-contract.json')))
        $asset = Join-Path $directory 'statistics.json'
        Invoke-KazeCargo -CargoArguments (@('run', '--locked', '--example', 'statistics_asset') + $networkArgs + @('--',
            'tests/fixtures/statistics/clean-contract.jsonl', 'authored-contract-v1', 'contract_fixture', 'CC0-1.0', $asset, '--dictionary', $dictionary) + $statisticsFlags)
        $assetHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $asset).Hash.ToLowerInvariant()
        $dictionaryHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $dictionary).Hash.ToLowerInvariant()
        $counts = [ordered]@{ morphemes = 0; oov = 0; review = 0; undetermined = 0; low_risk = 0; statistics_observed = 0 }
        foreach ($suffix in @('001', '002', '003')) {
            $inputPath = "evaluation/ppocrv6-medium-user-$suffix.jsonl"
            $reportPath = Join-Path $directory "recognition-$suffix.json"
            $morphologyPath = Join-Path $directory "morphology-$suffix.json"
            Invoke-KazeCargo -CargoArguments (@('run', '--locked', '--example', 'recognition_samples') + $networkArgs + @('--',
                $inputPath, $reportPath, $asset, $assetHash, 'contract_fixture', '--dictionary', $dictionary))
            Invoke-KazeCargo -CargoArguments (@('run', '--locked', '--example', 'dictionary_snapshot') + $networkArgs + @('--',
                $dictionary, $inputPath, $morphologyPath))
            Invoke-KazeCargo -CargoArguments (@('run', '--locked', '--no-default-features', '--example', 'recognition_quality') + $networkArgs + @('--',
                $inputPath, $reportPath, (Join-Path $directory "quality-$suffix.json")))
            $reports = Get-Content -Raw -Encoding UTF8 -LiteralPath $reportPath | ConvertFrom-Json
            $morphology = Get-Content -Raw -Encoding UTF8 -LiteralPath $morphologyPath | ConvertFrom-Json
            foreach ($report in $reports.reports) {
                $tokens = @($morphology.reports | Where-Object id -eq $report.segment_id)[0].morphemes
                $statistics = @($report.evidence | Where-Object kind -eq 'lexical_statistics')[0]
                $identity = @($report.provenance | Where-Object component -eq 'dictionary')[0]
                if ($identity.sha256 -ne $dictionaryHash -or $statistics.status -ne 'observed' -or
                    $report.recognition_risk.value -ne $null -or $report.metrics.slm_calls -ne 0 -or $report.decision -eq 'low_risk') {
                    throw 'Dictionary identity/evidence/abstention contract failed.'
                }
                if ($report.metrics.morpheme_count -ne $tokens.Count) { throw 'Snapshot and recognition segmentation differ.' }
                $oov = @($tokens | Where-Object is_oov).Count
                $counts.morphemes += $tokens.Count
                $counts.oov += $oov
                $counts[$report.decision] += 1
                $counts.statistics_observed += 1
                if (-not $allCases.ContainsKey($report.segment_id)) { $allCases[$report.segment_id] = [ordered]@{} }
                $allCases[$report.segment_id][$edition] = [ordered]@{
                    decision = $report.decision; morpheme_count = $tokens.Count; oov_count = $oov
                    surfaces = @($tokens | ForEach-Object surface)
                    spans = @($tokens | ForEach-Object span)
                    dictionary_forms = @($tokens | ForEach-Object dictionary_form)
                    readings = @($tokens | ForEach-Object reading)
                    part_of_speech = @($tokens | ForEach-Object { ,$_.part_of_speech })
                    string_features = $report.string_features
                    character_pairs = $statistics.value.character_pairs
                    words = $statistics.value.words
                    word_pairs = $statistics.value.word_pairs
                    pos_pairs = if ($WithPos) { $statistics.value.pos } else { $null }
                }
            }
        }
        # Separate authored hard-clean text probes; these are not real ASR measurements.
        Invoke-KazeCargo -CargoArguments (@('run', '--locked', '--example', 'dictionary_snapshot') + $networkArgs + @('--',
            $dictionary, 'tests/fixtures/statistics/hard-clean.jsonl', (Join-Path $directory 'hard-clean-morphology.json')))
        $hardClean = Get-Content -Raw -Encoding UTF8 -LiteralPath (Join-Path $directory 'hard-clean-morphology.json') | ConvertFrom-Json
        foreach ($row in $hardClean.reports) {
            if (-not $hardCleanCases.ContainsKey($row.id)) { $hardCleanCases[$row.id] = [ordered]@{} }
            $hardCleanCases[$row.id][$edition] = [ordered]@{
                morpheme_count = $row.morphemes.Count
                oov_count = @($row.morphemes | Where-Object is_oov).Count
                surfaces = @($row.morphemes | ForEach-Object surface)
                spans = @($row.morphemes | ForEach-Object span)
            }
        }
        $editions += [ordered]@{ edition = $edition; version = '20250129'; mode = 'C'
            dictionary_sha256 = $dictionaryHash; dictionary_bytes = (Get-Item -LiteralPath $dictionary).Length
            statistics_sha256 = $assetHash; counts = $counts }
    }
    $comparison = @()
    foreach ($id in @($allCases.Keys | Sort-Object)) {
        $variants = $allCases[$id]
        $stringBaseline = $variants.small.string_features | ConvertTo-Json -Depth 20 -Compress
        if ($null -eq $variants.small.string_features -or
            $stringBaseline -cne ($variants.core.string_features | ConvertTo-Json -Depth 20 -Compress) -or
            $stringBaseline -cne ($variants.full.string_features | ConvertTo-Json -Depth 20 -Compress)) {
            throw 'Raw string observations must be present and identical across dictionaries.'
        }
        $comparison += [ordered]@{ id = $id; variants = $variants
            segmentation_changed = (($variants.small.surfaces -join [char]31) -cne ($variants.core.surfaces -join [char]31) -or
                ($variants.small.surfaces -join [char]31) -cne ($variants.full.surfaces -join [char]31))
            oov_changed = ($variants.small.oov_count -ne $variants.core.oov_count -or $variants.small.oov_count -ne $variants.full.oov_count)
            decision_changed = ($variants.small.decision -ne $variants.core.decision -or $variants.small.decision -ne $variants.full.decision) }
    }
    $result = [ordered]@{ schema_version = 'kzn.dictionary.matrix.v1'; case_count = $comparison.Count
        pos_enabled = $WithPos.IsPresent; quality_accepted = $false; corpus_role = 'synthetic_contract_fixture_only'; gold_used_for_inference = $false
        editions = $editions; comparison = $comparison; hard_clean_comparison = $hardCleanCases }
    [IO.File]::WriteAllText((Join-Path $repoRoot "$outputRoot/summary.json"), ($result | ConvertTo-Json -Depth 20), (New-Object Text.UTF8Encoding($false)))
    $editions | ForEach-Object { Write-Host ("{0}: morphemes={1}, OOV={2}, review={3}, undetermined={4}" -f $_.edition, $_.counts.morphemes, $_.counts.oov, $_.counts.review, $_.counts.undetermined) }
    Write-Host "Local comparison saved: $outputRoot/summary.json"
} finally { Pop-Location }
