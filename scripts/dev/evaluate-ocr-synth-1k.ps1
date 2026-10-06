#Requires -Version 5.1
[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$DatasetRoot,[switch]$Offline)
. (Join-Path $PSScriptRoot 'common.ps1')
$repoRoot=Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Push-Location $repoRoot
try {
    if($env:CI -eq 'true'){throw 'User-derived evaluation is local only.'}
    & (Join-Path $PSScriptRoot 'prepare-ocr-synth-1k.ps1') -DatasetRoot $DatasetRoot -Offline:$Offline
    $out='target/kzn-ocr-synth-1k'
    $networkArgs=@();if($Offline){$networkArgs+='--offline'}
    # Only train references become clean statistics input. Keep original reference files for attribution.
    $train=@(Get-Content -Encoding UTF8 "$out/train.references.jsonl" | ForEach-Object {$_ | ConvertFrom-Json})
    if($train.Count -ne 500 -or @($train | Where-Object split -CNE 'train').Count){throw 'Expected 500 training rows only.'}
    $clean=@($train | ForEach-Object { [ordered]@{id=$_.id;document_id=$_.document_id;text=$_.transcription} | ConvertTo-Json -Compress })
    [IO.File]::WriteAllText((Join-Path $repoRoot "$out/train.clean.jsonl"),($clean -join "`n")+"`n",(New-Object Text.UTF8Encoding($false)))
    $summary=@()
    foreach($edition in @('small','core','full')){
        & (Join-Path $PSScriptRoot 'setup-sudachi.ps1') -Edition $edition -Offline:$Offline
        $dictionary=if($edition -eq 'small'){'resources/sudachi/system.dic'}else{"target/sudachi-dictionaries/$edition/system.dic"}
        $asset="$out/$edition/statistics.json"
        Invoke-KazeCargo -CargoArguments (@('run','--locked','--example','statistics_asset')+$networkArgs+@('--',"$out/train.clean.jsonl",'kzn-ocr-synth-1k.train.v1','ja.ocr.synth.v1','Mixed: see train.references.jsonl and ATTRIBUTION.md',$asset,'--dictionary',$dictionary,'--with-pos'))
        $hash=(Get-FileHash -LiteralPath $asset -Algorithm SHA256).Hash.ToLowerInvariant()
        foreach($policy in @('default','sparse')){
            $report="$out/$edition/development-$policy.json"
            $quality="$out/$edition/development-$policy-quality.json"
            $flags=@();if($policy -eq 'sparse'){$flags+='--sparse-review'}
            Invoke-KazeCargo -CargoArguments (@('run','--locked','--example','recognition_samples')+$networkArgs+@('--',"$out/development.inputs.jsonl",$report,$asset,$hash,'ja.ocr.synth.v1','--dictionary',$dictionary)+$flags)
            Invoke-KazeCargo -CargoArguments (@('run','--locked','--no-default-features','--example','recognition_quality')+$networkArgs+@('--',"$out/development.references.jsonl",$report,$quality,'--feature-ablation'))
            $result=Get-Content -Raw -Encoding UTF8 $quality | ConvertFrom-Json
            $summary+=[ordered]@{edition=$edition;policy=$policy;statistics_sha256=$hash;observation_sha256=$result.observation_sha256;reference_sha256=$result.reference_sha256;summary=$result.summary;feature_ablation=$result.feature_ablation}
        }
    }
    $result=[ordered]@{schema_version='kzn.ocr_synth.development.v1';quality_accepted=$false;training_split='train';evaluation_split='development';rows=$summary}
    [IO.File]::WriteAllText((Join-Path $repoRoot "$out/development-summary.json"),($result | ConvertTo-Json -Depth 30),(New-Object Text.UTF8Encoding($false)))
    Write-Host 'Three-dictionary development comparison complete. No calibration/test inference.'
} finally {Pop-Location}