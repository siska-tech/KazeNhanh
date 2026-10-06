#Requires -Version 5.1
[CmdletBinding()]
param([switch]$Offline)
. (Join-Path $PSScriptRoot 'common.ps1')
$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Push-Location $repoRoot
try {
    if ($env:CI -eq 'true') { throw 'User-derived observations are local only.' }
    $networkArgs = @(); if ($Offline) { $networkArgs += '--offline' }
    $out = 'target/sparse-review'
    New-Item -ItemType Directory -Force $out | Out-Null
    $summary = @()
    foreach ($edition in @('small','core','full')) {
        $dictionary = if ($edition -eq 'small') { 'resources/sudachi/system.dic' } else { "target/sudachi-dictionaries/$edition/system.dic" }
        $asset = "target/dictionary-matrix/$edition/statistics.json"
        if (-not (Test-Path -LiteralPath $asset)) { throw 'Run evaluate-dictionary-matrix.ps1 first to generate the three synthetic statistics assets.' }
        $assetHash = (Get-FileHash -LiteralPath $asset -Algorithm SHA256).Hash.ToLowerInvariant()
        $reviewIds = @(); $changedIds = @(); $unconfirmedReviewIds = @()
        $confirmedErrors = 0; $reviewedErrors = 0; $reviewedMatches = 0; $confirmedMatches = 0
        foreach ($suffix in @('001','002','003')) {
            $inputPath = "evaluation/ppocrv6-medium-user-$suffix.jsonl"
            $report = "$out/$edition-$suffix.json"
            $quality = "$out/$edition-$suffix-quality.json"
            Invoke-KazeCargo -CargoArguments (@('run','--locked','--example','recognition_samples') + $networkArgs + @('--',
                $inputPath,$report,$asset,$assetHash,'contract_fixture','--dictionary',$dictionary,'--sparse-review'))
            Invoke-KazeCargo -CargoArguments (@('run','--locked','--no-default-features','--example','recognition_quality') + $networkArgs + @('--',$inputPath,$report,$quality))
            $after = Get-Content -LiteralPath $report -Raw -Encoding UTF8 | ConvertFrom-Json
            $before = Get-Content -LiteralPath "target/dictionary-matrix/$edition/recognition-$suffix.json" -Raw -Encoding UTF8 | ConvertFrom-Json
            foreach ($row in $after.reports) {
                $previous = @($before.reports | Where-Object segment_id -eq $row.segment_id)
                if ($previous.Count -ne 1 -or $previous[0].original_text -cne $row.original_text -or
                    $previous[0].recognizer_evidence.confidences[0].score.value -ne $row.recognizer_evidence.confidences[0].score.value) { throw 'Before/after identity or raw confidence differs.' }
                if ($row.decision -eq 'review') { $reviewIds += $row.segment_id }
                if ($row.decision -ne $previous[0].decision) { $changedIds += $row.segment_id }
            }
            $q = (Get-Content -LiteralPath $quality -Raw -Encoding UTF8 | ConvertFrom-Json).summary
            $confirmedErrors += $q.confirmed_mismatch_count; $confirmedMatches += $q.confirmed_match_count
            $reviewedErrors += $q.decision_counts.review[1]; $reviewedMatches += $q.decision_counts.review[0]
            $unconfirmedReviewIds += $q.decision_counts.review[2]
        }
        $summary += [ordered]@{edition=$edition;review_ids=$reviewIds;changed_ids=$changedIds
            confirmed_errors=$confirmedErrors;reviewed_confirmed_errors=$reviewedErrors
            confirmed_matches=$confirmedMatches;reviewed_confirmed_matches=$reviewedMatches
            unconfirmed_review_count=($unconfirmedReviewIds | Measure-Object -Sum).Sum}
    }
    $result = [ordered]@{schema_version='kzn.sparse_review.observation.v1';quality_accepted=$false
        statistics_corpus_role='synthetic_contract_not_representative';editions=$summary}
    [IO.File]::WriteAllText((Join-Path $repoRoot "$out/summary.json"),($result | ConvertTo-Json -Depth 8),(New-Object Text.UTF8Encoding($false)))
    $summary | ConvertTo-Json -Depth 6 | Write-Host
} finally { Pop-Location }