#Requires -Version 5.1
[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$DatasetRoot,[switch]$Offline)
. (Join-Path $PSScriptRoot 'common.ps1')
$repoRoot=Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Push-Location $repoRoot
try {
    if($env:CI -eq 'true'){throw 'User-provided dataset preparation is local only.'}
    $lock=Get-Content -Raw -Encoding UTF8 resources/evaluation/kzn-ocr-synth-1k.lock.json | ConvertFrom-Json
    foreach($file in $lock.files){
        $path=Join-Path $DatasetRoot $file.path
        if(-not(Test-Path -LiteralPath $path) -or (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() -cne $file.sha256){throw "Missing or changed dataset file: $($file.path)"}
    }
    $dataset=Join-Path $DatasetRoot 'dataset/kzn-ocr-synth-1k.jsonl'
    $manifest=Join-Path $DatasetRoot 'dataset/kzn-ocr-synth-1k.manifest.json'
    $output='target/kzn-ocr-synth-1k'
    $networkArgs=@();if($Offline){$networkArgs+='--offline'}
    Invoke-KazeCargo -CargoArguments (@('run','--locked','--no-default-features','--example','validate_recognition_dataset')+$networkArgs+@('--',$manifest,$dataset,"$output/audit.json"))
    $declaration=Get-Content -Raw -Encoding UTF8 -LiteralPath $manifest | ConvertFrom-Json
    $byId=@{};foreach($item in $declaration.items){$byId[$item.id]=$item}
    $rows=@(Get-Content -Encoding UTF8 -LiteralPath $dataset | Where-Object { $_.Trim() } | ForEach-Object { $_ | ConvertFrom-Json })
    foreach($row in $rows){if($row.split -cne $byId[$row.id].split){throw 'Row and manifest split mismatch.'}}
    $summary=@()
    foreach($split in @('train','calibration','test','development')){
        $selected=@($rows | Where-Object split -CEQ $split)
        $references=@();$inputs=@()
        foreach($row in $selected){
            $references+=($row | ConvertTo-Json -Depth 30 -Compress)
            # Explicit allowlist: no transcription, labels, edit distance, CER or render metadata.
            $input=[ordered]@{id=$row.id;source=$row.source;document_id=$row.document_id;text=$row.text;engine=$row.engine;confidence=$row.confidence;confidence_scale=$row.confidence_scale}
            $inputs+=($input | ConvertTo-Json -Depth 5 -Compress)
        }
        $referencePath=Join-Path $repoRoot "$output/$split.references.jsonl"
        $inputPath=Join-Path $repoRoot "$output/$split.inputs.jsonl"
        [IO.File]::WriteAllText($referencePath,($references -join "`n")+"`n",(New-Object Text.UTF8Encoding($false)))
        [IO.File]::WriteAllText($inputPath,($inputs -join "`n")+"`n",(New-Object Text.UTF8Encoding($false)))
        $summary+=[ordered]@{split=$split;count=$selected.Count;reference_sha256=(Get-FileHash $referencePath -Algorithm SHA256).Hash.ToLowerInvariant();input_sha256=(Get-FileHash $inputPath -Algorithm SHA256).Hash.ToLowerInvariant()}
    }
    Copy-Item -LiteralPath (Join-Path $DatasetRoot 'ATTRIBUTION.md') -Destination "$output/ATTRIBUTION.md"
    $result=[ordered]@{dataset_id=$lock.dataset_id;data_kind='synthetic';quality_accepted=$false;inference_executed=$false;splits=$summary}
    [IO.File]::WriteAllText((Join-Path $repoRoot "$output/preparation.json"),($result|ConvertTo-Json -Depth 6),(New-Object Text.UTF8Encoding($false)))
    Write-Host "Prepared local split inputs/references: $output"
} finally {Pop-Location}