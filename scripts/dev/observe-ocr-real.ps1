#Requires -Version 5.1
[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$DatasetRoot,[switch]$Offline)
. (Join-Path $PSScriptRoot 'common.ps1')
$repoRoot=Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Push-Location $repoRoot
try {
    if($env:CI -eq 'true'){throw 'Real OCR observation is local only.'}
    & (Join-Path $PSScriptRoot 'prepare-ocr-real.ps1') -DatasetRoot $DatasetRoot -Offline:$Offline
    $out='target/kzn-ocr-real';$net=@();if($Offline){$net+='--offline'}
    $declared=@(Get-Content -Encoding UTF8 "$out/development.declared-references.jsonl"|ForEach-Object{$_|ConvertFrom-Json})
    $inputs=@{};Get-Content -Encoding UTF8 "$out/development.inputs.jsonl"|ForEach-Object{$r=$_|ConvertFrom-Json;$inputs[$r.id]=$r}
    $refs=@{};Get-Content -Encoding UTF8 "$out/development.review-references.jsonl"|ForEach-Object{$r=$_|ConvertFrom-Json;$refs[$r.id]=$r}
    $sourceLock=Get-Content -Raw -Encoding UTF8 (Join-Path $DatasetRoot 'dataset-real/sources.lock.json')|ConvertFrom-Json
    $runnerHash=(Get-FileHash (Join-Path $DatasetRoot 'ocr-runner/src/main.rs') -Algorithm SHA256).Hash.ToLowerInvariant()
    $enc=New-Object Text.UTF8Encoding($false);$matrix=@()
    foreach($source in @('ndl_oneline','gov_pdf','commons_scene')){
        $dir="$out/$source";New-Item -ItemType Directory -Force $dir|Out-Null
        $ids=@($declared|Where-Object source_set -CEQ $source|ForEach-Object{$_.id})
        $input=@($ids|ForEach-Object{$inputs[$_]|ConvertTo-Json -Depth 8 -Compress});$ref=@($ids|ForEach-Object{$refs[$_]|ConvertTo-Json -Depth 12 -Compress})
        [IO.File]::WriteAllText((Join-Path $repoRoot "$dir/inputs.jsonl"),($input -join "`n")+"`n",$enc)
        [IO.File]::WriteAllText((Join-Path $repoRoot "$dir/review-references.jsonl"),($ref -join "`n")+"`n",$enc)
        $d=[ordered]@{schema_version='kzn.ocr.ctc_description.v1';expected_engine='pure-onnx-ocr 0.2.1 / PP-OCRv6 medium (det+rec)';expected_scale='mean CTC emitted-token probability, char-weighted over boxes; null when no text';recognizer=[ordered]@{engine='pure-onnx-ocr';model='PP-OCRv6 medium det+rec (producer declared; model hash unavailable in real lock)';version="0.2.1+$($sourceLock.pure_onnx_ocr_git)";decoder="ctc-greedy:$($sourceLock.pure_onnx_ocr_git):runner:$runnerHash"};profile=[ordered]@{id="ja.ocr.real.$source.ctc.v1";source='ocr';transcription_policy_id='raw.v1';required_signals=@('confidence')};domain="ja.ocr.real.$source.v1"}
        [IO.File]::WriteAllText((Join-Path $repoRoot "$dir/description.json"),($d|ConvertTo-Json -Depth 8),$enc)
        foreach($edition in @('small','core','full')){
            $dictionary=if($edition -eq 'small'){'resources/sudachi/system.dic'}else{"target/sudachi-dictionaries/$edition/system.dic"}
            $report="$dir/$edition.reports.json";$quality="$dir/$edition.quality.json"
            Invoke-KazeCargo -CargoArguments (@('run','--locked','--example','recognition_samples')+$net+@('--',"$dir/inputs.jsonl",$report,'--source-description',"$dir/description.json",'--dictionary',$dictionary))
            Invoke-KazeCargo -CargoArguments (@('run','--locked','--no-default-features','--example','recognition_quality')+$net+@('--',"$dir/review-references.jsonl",$report,$quality))
            $v=Get-Content -Raw -Encoding UTF8 $report|ConvertFrom-Json
            $q=Get-Content -Raw -Encoding UTF8 $quality|ConvertFrom-Json
            if($v.reports.Count -ne $ids.Count -or $q.summary.confirmed_count -ne 0 -or $q.summary.unconfirmed_count -ne $ids.Count -or $null -ne $q.summary.confirmed_metrics.review_precision -or $null -ne $q.summary.confirmed_metrics.review_recall){throw 'Reference/count contract mismatch'}
            foreach($r in $v.reports){$raw=$inputs[$r.segment_id];$c=$r.recognizer_evidence.confidences[0];if($r.original_text -cne $raw.text -or $r.domain -cne $d.domain -or $r.recognizer_evidence.profile.id -cne $d.profile.id -or $r.metrics.slm_calls -ne 0 -or $null -ne $r.recognition_risk.value -or $r.decision -ceq 'low_risk'){throw 'Observation contract mismatch'};if($null -eq $raw.confidence){if($c.status -cne 'missing' -or $null -ne $c.score){throw 'Missingness changed'}}else{if($c.score.value -ne $raw.confidence -or $c.score.direction -cne 'higher_is_better' -or $null -ne $c.score.calibration_id){throw 'Raw confidence changed'}}}
            $matrix+=[ordered]@{source_set=$source;edition=$edition;count=$v.reports.Count;review=@($v.reports|Where-Object decision -CEQ 'review').Count;undetermined=@($v.reports|Where-Object decision -CEQ 'undetermined').Count;report_sha256=(Get-FileHash $report -Algorithm SHA256).Hash.ToLowerInvariant();description_sha256=(Get-FileHash "$dir/description.json" -Algorithm SHA256).Hash.ToLowerInvariant()}
        }
    }
    $summary=[ordered]@{schema_version='kzn.real_ocr.observation_matrix.v1';quality_accepted=$false;confirmed_gold_count=0;statistics_enabled=$false;source_threshold_enabled=$false;slm_calls=0;rows=$matrix}
    [IO.File]::WriteAllText((Join-Path $repoRoot "$out/observation-summary.json"),($summary|ConvertTo-Json -Depth 8),$enc)
}finally{Pop-Location}