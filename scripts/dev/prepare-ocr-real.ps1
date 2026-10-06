#Requires -Version 5.1
[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$DatasetRoot,[switch]$Offline)
. (Join-Path $PSScriptRoot 'common.ps1')
$repoRoot=Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Push-Location $repoRoot
try {
    if($env:CI -eq 'true'){throw 'User-provided real OCR data is local only.'}
    $lock=Get-Content -Raw -Encoding UTF8 resources/evaluation/kzn-ocr-real.lock.json|ConvertFrom-Json
    foreach($f in $lock.files){if((Get-FileHash (Join-Path $DatasetRoot $f.path) -Algorithm SHA256).Hash.ToLowerInvariant() -cne $f.sha256){throw "Changed dataset file: $($f.path)"}}
    $dataset=Join-Path $DatasetRoot 'dataset-real/kzn-ocr-real.jsonl'
    $manifest=Join-Path $DatasetRoot 'dataset-real/kzn-ocr-real.manifest.json'
    $out='target/kzn-ocr-real';New-Item -ItemType Directory -Force $out|Out-Null
    $net=@();if($Offline){$net+='--offline'}
    Invoke-KazeCargo -CargoArguments (@('run','--locked','--no-default-features','--example','validate_recognition_dataset')+$net+@('--',$manifest,$dataset,"$out/audit.json"))
    $m=Get-Content -Raw -Encoding UTF8 $manifest|ConvertFrom-Json
    $byId=@{};foreach($item in $m.items){$byId[$item.id]=$item}
    $rows=@(Get-Content -Encoding UTF8 $dataset|ForEach-Object{$_|ConvertFrom-Json})
    $counts=@{};$families=@{}
    $references=@();$inputs=@();$review=@()
    foreach($row in $rows){
        if($row.split -cne $byId[$row.id].split){throw 'Row/manifest split mismatch'}
        if($row.source -cne 'ocr' -or $row.comparison_policy -cne 'raw.v1'){throw 'Unsupported source/comparison policy'}
        $key="$($row.source_set)/$($row.split)/$($row.transcription_status)";if(-not $counts.ContainsKey($key)){$counts[$key]=0};$counts[$key]++
        # Source-file grouping complements the manifest's declared page/document groups.
        $sourceFile=$row.PSObject.Properties['source_file']
        if($null -ne $sourceFile -and -not [string]::IsNullOrWhiteSpace([string]$sourceFile.Value)){
            $family="$($row.source_set)/$($sourceFile.Value)"
            if(-not $families.ContainsKey($family)){$families[$family]=@{}}
            $families[$family][$row.split]=$true
        }
        if($row.split -cne 'development'){continue}
        $reason=switch -CaseSensitive ($row.source_set){
            'ndl_oneline' {'shinjitai_reference_requires_comparison_policy_review'}
            'gov_pdf' {'pdf_text_layer_image_alignment_not_human_verified'}
            'commons_scene' {'model_transcription_not_human_verified'}
            default {throw 'Unknown source_set'}
        }
        if($row.transcription_status -cnotin @('verified','unconfirmed')){throw 'Unknown reference status'}
        # Preserve the producer's complete row separately; it is not an accepted runtime gold label.
        $references+=($row|ConvertTo-Json -Depth 30 -Compress)
        $input=[ordered]@{id=$row.id;source=$row.source;document_id=$row.document_id;text=$row.text;engine=$row.engine;confidence=$row.confidence;confidence_scale=$row.confidence_scale}
        $inputs+=($input|ConvertTo-Json -Depth 8 -Compress)
        # Review-only projection: existing quality runner excludes unconfirmed rows from precision/recall.
        $review+=([ordered]@{id=$row.id;source=$row.source;document_id=$row.document_id;text=$row.text;transcription=$row.transcription;transcription_status='unconfirmed';comparison_policy=$row.comparison_policy;split=$row.split;source_set=$row.source_set;declared_transcription_status=$row.transcription_status;reference_basis=$row.reference_basis;review_reason=$reason}|ConvertTo-Json -Depth 8 -Compress)
    }
    $encoding=New-Object Text.UTF8Encoding($false)
    $outputs=@(@('development.declared-references.jsonl',$references),@('development.inputs.jsonl',$inputs),@('development.review-references.jsonl',$review));$hashes=@()
    foreach($pair in $outputs){$path=Join-Path $repoRoot "$out/$($pair[0])";[IO.File]::WriteAllText($path,($pair[1] -join "`n")+"`n",$encoding);$hashes+=[ordered]@{path=$pair[0];sha256=(Get-FileHash $path -Algorithm SHA256).Hash.ToLowerInvariant()}}
    Copy-Item -LiteralPath (Join-Path $DatasetRoot 'ATTRIBUTION-real.md') -Destination "$out/ATTRIBUTION-real.md"
    $overlap=@();foreach($key in @($families.Keys|Sort-Object)){if($families[$key].Count -gt 1){$overlap+=[ordered]@{source_file=$key;splits=@($families[$key].Keys|Sort-Object)}}}
    $result=[ordered]@{schema_version='kzn.real_ocr.preparation.v1';dataset_id=$lock.dataset_id;data_kind='measured';counts=$counts;development_count=$inputs.Count;quality_accepted=$false;inference_executed=$false;images_hash_verified=$false;reference_policy_review_required=$true;source_file_cross_split=$overlap;outputs=$hashes}
    [IO.File]::WriteAllText((Join-Path $repoRoot "$out/preparation.json"),($result|ConvertTo-Json -Depth 12),$encoding)
    Write-Host "Prepared $($inputs.Count) review-only development rows; no train/calibration/test projection or inference."
}finally{Pop-Location}