#Requires -Version 5.1
[CmdletBinding()]
param([switch]$Offline)
. (Join-Path $PSScriptRoot 'common.ps1')
$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Push-Location $repoRoot
try {
    if ($env:CI -eq 'true') { throw 'User-derived observations are local only.' }
    $networkArgs = @(); if ($Offline) { $networkArgs += '--offline' }
    $out = 'target/feature-ablation'
    New-Item -ItemType Directory -Force $out | Out-Null
    $editions = @()
    foreach ($edition in @('small','core','full')) {
        $totals = [ordered]@{}
        $inputs = @()
        foreach ($suffix in @('001','002','003')) {
            $reports = "target/dictionary-pos-matrix/$edition/recognition-$suffix.json"
            if (-not (Test-Path -LiteralPath $reports)) { throw 'Run evaluate-dictionary-matrix.ps1 -Offline -WithPos first.' }
            $resultPath = "$out/$edition-$suffix.json"
            Invoke-KazeCargo -CargoArguments (@('run','--locked','--no-default-features','--example','recognition_quality') + $networkArgs + @('--',
                "evaluation/ppocrv6-medium-user-$suffix.jsonl",$reports,$resultPath,'--feature-ablation'))
            $result = Get-Content -Raw -Encoding UTF8 -LiteralPath $resultPath | ConvertFrom-Json
            $inputs += [ordered]@{file=$resultPath;reference_sha256=$result.reference_sha256;observation_sha256=$result.observation_sha256}
            foreach ($probe in $result.feature_ablation.probes.PSObject.Properties) {
                if (-not $totals.Contains($probe.Name)) { $totals[$probe.Name] = [ordered]@{flagged=@(0,0,0);not_flagged=@(0,0,0);unavailable=@(0,0,0)} }
                foreach ($row in @('flagged','not_flagged','unavailable')) {
                    for ($i=0; $i -lt 3; $i++) { $totals[$probe.Name][$row][$i] += $probe.Value.$row[$i] }
                }
            }
        }
        $editions += [ordered]@{edition=$edition;inputs=$inputs;probes=$totals}
    }
    $summary=[ordered]@{schema_version='kzn.feature_ablation.observation.v1';quality_accepted=$false;runtime_policy_changed=$false
        columns=@('confirmed_match','confirmed_mismatch','unconfirmed');editions=$editions}
    [IO.File]::WriteAllText((Join-Path $repoRoot "$out/summary.json"),($summary | ConvertTo-Json -Depth 20),(New-Object Text.UTF8Encoding($false)))
    Write-Host "Local feature comparison saved: $out/summary.json"
} finally { Pop-Location }