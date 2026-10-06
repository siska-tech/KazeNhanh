#Requires -Version 5.1
[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$DatasetRoot,[switch]$Offline,[switch]$UsePreparedSnapshot)
. (Join-Path $PSScriptRoot 'common.ps1')
$repoRoot=Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Push-Location $repoRoot
try {
    if($env:CI -eq 'true'){throw 'User-derived OOF evaluation is local only.'}
    if($UsePreparedSnapshot){
        # Frozen, previously audited projections: never reinterpret them using an updated producer.
        $lock=Get-Content -Raw -Encoding UTF8 resources/evaluation/kzn-ocr-synth-1k.lock.json|ConvertFrom-Json
        foreach($f in $lock.files|Where-Object{$_.path -ne 'README.md' -and $_.path -ne 'ocr-runner/src/main.rs'}){
            if((Get-FileHash (Join-Path $DatasetRoot $f.path) -Algorithm SHA256).Hash.ToLowerInvariant() -cne $f.sha256){throw "Dataset snapshot mismatch: $($f.path)"}
        }
        $snapshot=Get-Content -Raw -Encoding UTF8 resources/evaluation/kzn-ocr-synth-1k-oof.snapshot.json|ConvertFrom-Json
        foreach($f in $snapshot.files){
            if((Get-FileHash "target/kzn-ocr-synth-1k/$($f.path)" -Algorithm SHA256).Hash.ToLowerInvariant() -cne $f.sha256){throw "Prepared snapshot mismatch: $($f.path)"}
        }
    }else{
        & (Join-Path $PSScriptRoot 'evaluate-source-synth-1k.ps1') -DatasetRoot $DatasetRoot -Offline:$Offline
    }
    $out='target/kzn-ocr-synth-1k';$oof="$out/oof"
    $networkArgs=@();if($Offline){$networkArgs+='--offline'}
    $fusion=@('run','--locked','--no-default-features','--example','fusion_baseline')+$networkArgs+@('--')
    Invoke-KazeCargo -CargoArguments ($fusion+@('prepare-oof',"$out/train.references.jsonl",(Join-Path $DatasetRoot 'dataset/kzn-ocr-synth-1k.manifest.json'),$oof))
    $rows=@()
    foreach($edition in @('small','core','full')){
        $dictionary=if($edition -eq 'small'){'resources/sudachi/system.dic'}else{"target/sudachi-dictionaries/$edition/system.dic"}
        New-Item -ItemType Directory -Force "$oof/$edition"|Out-Null
        $foldReports=@()
        foreach($part in @('fold0','fold1','fold2','fold3','fold4','full')){
            $asset="$oof/$edition/$part.statistics.json"
            $report="$oof/$edition/$part.reports.json"
            $input=if($part -eq 'full'){"$out/development.inputs.jsonl"}else{"$oof/$part.inputs.jsonl"}
            Invoke-KazeCargo -CargoArguments (@('run','--locked','--example','statistics_asset')+$networkArgs+@('--',"$oof/$part.clean.jsonl","kzn.ocr.oof.$part.v1",'ja.ocr.synth.v1','Mixed; see local ATTRIBUTION.md',$asset,'--dictionary',$dictionary,'--with-pos'))
            $hash=(Get-FileHash $asset -Algorithm SHA256).Hash.ToLowerInvariant()
            Invoke-KazeCargo -CargoArguments (@('run','--locked','--example','recognition_samples')+$networkArgs+@('--',$input,$report,$asset,$hash,'ja.ocr.synth.v1','--source-rule',"$out/source-mapping.json",'--dictionary',$dictionary))
            if($part -ne 'full'){$foldReports+=$report}
        }
        $merged="$oof/$edition/train.reports.json";$result="$oof/$edition/fusion-development.json"
        # Rust serialization preserves f64 round-trip; do not merge reports via PowerShell JSON.
        Invoke-KazeCargo -CargoArguments ($fusion+@('merge-oof',$merged)+$foldReports)
        Invoke-KazeCargo -CargoArguments ($fusion+@("$out/train.references.jsonl",$merged,"$out/development.references.jsonl","$oof/$edition/full.reports.json",$result,'--oof-plan',"$oof/plan.json"))
        $evaluation=Get-Content -Raw -Encoding UTF8 $result|ConvertFrom-Json
        $ablations=@();foreach($r in $evaluation.results){$ablations+=[ordered]@{ablation=$r.ablation;flagged_matches=$r.development.flagged_matches;flagged_mismatches=$r.development.flagged_mismatches;unflagged_matches=$r.development.unflagged_matches;unflagged_mismatches=$r.development.unflagged_mismatches;precision=$r.development.precision;recall=$r.development.recall;false_flag_rate=$r.development.false_flag_rate}}
        $rows+=[ordered]@{edition=$edition;results=$ablations;output_sha256=(Get-FileHash $result -Algorithm SHA256).Hash.ToLowerInvariant()}
    }
    $summary=[ordered]@{schema_version='kzn.fusion.oof_matrix.v1';quality_accepted=$false;plan_sha256=(Get-FileHash "$oof/plan.json" -Algorithm SHA256).Hash.ToLowerInvariant();rows=$rows}
    [IO.File]::WriteAllText((Join-Path $repoRoot "$oof/summary.json"),($summary|ConvertTo-Json -Depth 12),(New-Object Text.UTF8Encoding($false)))
    Write-Host 'OOF development comparison complete; calibration/test and runtime policy unchanged.'
}finally{Pop-Location}