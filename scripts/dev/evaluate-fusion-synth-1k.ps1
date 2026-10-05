#Requires -Version 5.1
[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$DatasetRoot,[switch]$Offline)
. (Join-Path $PSScriptRoot 'common.ps1')
$repoRoot=Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Push-Location $repoRoot
try {
    if($env:CI -eq 'true'){throw 'User-derived fusion evaluation is local only.'}
    # Refresh audited splits/source definition and verify train-only statistics before consuming outputs.
    & (Join-Path $PSScriptRoot 'evaluate-source-synth-1k.ps1') -DatasetRoot $DatasetRoot -Offline:$Offline
    $out='target/kzn-ocr-synth-1k';$networkArgs=@();if($Offline){$networkArgs+='--offline'}
    $rows=@()
    foreach($edition in @('small','core','full')){
        $dictionary=if($edition -eq 'small'){'resources/sudachi/system.dic'}else{"target/sudachi-dictionaries/$edition/system.dic"}
        $asset="$out/$edition/statistics.json";$hash=(Get-FileHash $asset -Algorithm SHA256).Hash.ToLowerInvariant()
        $trainReport="$out/$edition/train-source.json";$result="$out/$edition/fusion-development.json"
        Invoke-KazeCargo -CargoArguments (@('run','--locked','--example','recognition_samples')+$networkArgs+@('--',"$out/train.inputs.jsonl",$trainReport,$asset,$hash,'ja.ocr.synth.v1','--source-rule',"$out/source-mapping.json",'--dictionary',$dictionary))
        Invoke-KazeCargo -CargoArguments (@('run','--locked','--no-default-features','--example','fusion_baseline')+$networkArgs+@('--',"$out/train.references.jsonl",$trainReport,"$out/development.references.jsonl","$out/$edition/source-default.json",$result))
        $evaluation=Get-Content -Raw -Encoding UTF8 $result|ConvertFrom-Json
        $ablations=@();foreach($r in $evaluation.results){$ablations+=[ordered]@{ablation=$r.ablation;flagged_matches=$r.development.flagged_matches;flagged_mismatches=$r.development.flagged_mismatches;unflagged_matches=$r.development.unflagged_matches;unflagged_mismatches=$r.development.unflagged_mismatches;precision=$r.development.precision;recall=$r.development.recall;false_flag_rate=$r.development.false_flag_rate}}
        $rows+=[ordered]@{edition=$edition;results=$ablations;output_sha256=(Get-FileHash $result -Algorithm SHA256).Hash.ToLowerInvariant()}
    }
    $summary=[ordered]@{schema_version='kzn.fusion.matrix.v1';quality_accepted=$false;rows=$rows}
    [IO.File]::WriteAllText((Join-Path $repoRoot "$out/fusion-summary.json"),($summary|ConvertTo-Json -Depth 12),(New-Object Text.UTF8Encoding($false)))
    Write-Host 'Fusion development comparison complete; no runtime policy changes or calibration/test inference.'
}finally{Pop-Location}