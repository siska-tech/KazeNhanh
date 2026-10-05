#Requires -Version 5.1
[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$DatasetRoot,[switch]$Offline)
. (Join-Path $PSScriptRoot 'common.ps1')
$repoRoot=Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Push-Location $repoRoot
try {
    if($env:CI -eq 'true'){throw 'User-derived evaluation is local only.'}
    & (Join-Path $PSScriptRoot 'prepare-ocr-synth-1k.ps1') -DatasetRoot $DatasetRoot -Offline:$Offline
    $out='target/kzn-ocr-synth-1k';$networkArgs=@();if($Offline){$networkArgs+='--offline'}
    $train=@(Get-Content -Encoding UTF8 "$out/train.inputs.jsonl"|ForEach-Object{$_|ConvertFrom-Json})
    $expectedEngine='pure-onnx-ocr 0.2.1 / PP-OCRv6 medium (det+rec)'
    $expectedScale='mean CTC emitted-token probability, char-weighted over boxes; null when no text'
    if($train.Count -ne 500){throw 'Expected 500 train inputs.'}
    foreach($row in $train){if($row.engine -cne $expectedEngine -or $row.confidence_scale -cne $expectedScale){throw 'Training engine/scale mismatch'}}
    $scores=@($train|Where-Object{$null -ne $_.confidence}|ForEach-Object{[double]$_.confidence}|Sort-Object)
    if($scores.Count -eq 0){throw 'No observed training scores'}
    # Fixed label-free lower decile, nearest rank; strict < boundary at inference.
    $rank=[int][Math]::Ceiling(0.1*$scores.Count);$threshold=$scores[$rank-1]
    $trainHash=(Get-FileHash "$out/train.inputs.jsonl" -Algorithm SHA256).Hash.ToLowerInvariant()
    $sourceLock=Get-Content -Raw -Encoding UTF8 (Join-Path $DatasetRoot 'dataset/sources.lock.json')|ConvertFrom-Json
    $runnerHash=(Get-FileHash (Join-Path $DatasetRoot 'ocr-runner/src/main.rs') -Algorithm SHA256).Hash.ToLowerInvariant()
    $model='ppocrv6.medium.det-rec:'+$sourceLock.models.'pure-onnx-ocr/tests/fixtures/models/ppocrv6/medium_det/inference.onnx'+':'+$sourceLock.models.'pure-onnx-ocr/tests/fixtures/models/ppocrv6/medium_rec/inference.onnx'
    $mapping=[ordered]@{expected_engine=$expectedEngine;expected_scale=$expectedScale;rule=[ordered]@{
        id="kzn.synth.train.lower-decile.v1.$($trainHash.Substring(0,16))"
        recognizer=[ordered]@{engine='pure-onnx-ocr';model=$model;version="0.2.1+$($sourceLock.pure_onnx_ocr_git)";decoder="ctc-greedy:$($sourceLock.pure_onnx_ocr_git):runner:$runnerHash"}
        profile=[ordered]@{id='ja.ocr.synth.ctc.v1';source='ocr';transcription_policy_id='raw.v1';required_signals=@('confidence')}
        domain='ja.ocr.synth.v1';granularity='segment';aggregation='ctc-emitted-mean.box-char-weighted.v1'
        threshold=[ordered]@{value=$threshold;meaning='engine_score';direction='higher_is_better';range=@(0.0,1.0);calibration_id=$null;target='emitted-token-confidence.box-char-weighted'}
    }}
    $mappingPath="$out/source-mapping.json"
    [IO.File]::WriteAllText((Join-Path $repoRoot $mappingPath),($mapping|ConvertTo-Json -Depth 15),(New-Object Text.UTF8Encoding($false)))
    $trainReferences=@(Get-Content -Encoding UTF8 "$out/train.references.jsonl"|ForEach-Object{$_|ConvertFrom-Json})
    if($trainReferences.Count -ne 500 -or @($trainReferences|Where-Object split -CNE 'train').Count){throw 'Expected train references only'}
    $clean=@($trainReferences|ForEach-Object{[ordered]@{id=$_.id;document_id=$_.document_id;text=$_.transcription}|ConvertTo-Json -Compress})
    $sha=[Security.Cryptography.SHA256]::Create()
    try {$expectedCleanHash=([BitConverter]::ToString($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes(($clean -join "`n")+"`n")))).Replace('-','').ToLowerInvariant()}finally{$sha.Dispose()}
    $rows=@()
    foreach($edition in @('small','core','full')){
        $dictionary=if($edition -eq 'small'){'resources/sudachi/system.dic'}else{"target/sudachi-dictionaries/$edition/system.dic"}
        $asset="$out/$edition/statistics.json"
        if(-not(Test-Path $asset)){throw 'Run evaluate-ocr-synth-1k.ps1 first to build train-only statistics.'}
        $artifact=Get-Content -Raw -Encoding UTF8 $asset|ConvertFrom-Json
        $cleanHash=(Get-FileHash "$out/train.clean.jsonl" -Algorithm SHA256).Hash.ToLowerInvariant()
        if($artifact.document_count -ne 500 -or $cleanHash -cne $expectedCleanHash -or $artifact.metadata.corpus_sha256 -cne $expectedCleanHash){throw 'Statistics training corpus mismatch'}
        $hash=(Get-FileHash $asset -Algorithm SHA256).Hash.ToLowerInvariant()
        foreach($policy in @('default','sparse')){
            $report="$out/$edition/source-$policy.json";$quality="$out/$edition/source-$policy-quality.json"
            $flags=@();if($policy -eq 'sparse'){$flags+='--sparse-review'}
            Invoke-KazeCargo -CargoArguments (@('run','--locked','--example','recognition_samples')+$networkArgs+@('--',"$out/development.inputs.jsonl",$report,$asset,$hash,'ja.ocr.synth.v1','--source-rule',$mappingPath,'--dictionary',$dictionary)+$flags)
            Invoke-KazeCargo -CargoArguments (@('run','--locked','--no-default-features','--example','recognition_quality')+$networkArgs+@('--',"$out/development.references.jsonl",$report,$quality))
            $q=Get-Content -Raw -Encoding UTF8 $quality|ConvertFrom-Json
            $rows+=[ordered]@{edition=$edition;policy=$policy;summary=$q.summary;base_summary=$q.base_summary;observation_sha256=$q.observation_sha256;reference_sha256=$q.reference_sha256;statistics_sha256=$hash}
        }
    }
    $result=[ordered]@{method='train_observed_confidence_lower_decile.nearest_rank.v1';threshold=$threshold;observed_train_count=$scores.Count;missing_train_count=(500-$scores.Count);rank=$rank;training_input_sha256=$trainHash;mapping_sha256=(Get-FileHash $mappingPath -Algorithm SHA256).Hash.ToLowerInvariant();quality_accepted=$false;rows=$rows}
    [IO.File]::WriteAllText((Join-Path $repoRoot "$out/source-summary.json"),($result|ConvertTo-Json -Depth 20),(New-Object Text.UTF8Encoding($false)))
    Write-Host 'Source development comparison complete; no calibration/test inference.'
}finally{Pop-Location}