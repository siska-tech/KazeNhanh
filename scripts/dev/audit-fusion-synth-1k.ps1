#Requires -Version 5.1
[CmdletBinding()]
param([switch]$Offline,[ValidateRange(3,20)][int]$Repetitions=3)
. (Join-Path $PSScriptRoot 'common.ps1')
$repoRoot=Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Push-Location $repoRoot
try {
    if($env:CI -eq 'true'){throw 'User-derived scope and CPU evaluation is local only.'}
    $out='target/kzn-ocr-synth-1k';$oof="$out/oof";$dest="$oof/cpu"
    New-Item -ItemType Directory -Force $dest|Out-Null
    $snapshot=Get-Content -Raw -Encoding UTF8 resources/evaluation/kzn-ocr-synth-1k-oof.snapshot.json|ConvertFrom-Json
    foreach($f in $snapshot.files){if((Get-FileHash "$out/$($f.path)" -Algorithm SHA256).Hash.ToLowerInvariant() -cne $f.sha256){throw 'Prepared snapshot changed'}}
    $lock=Get-Content -Raw -Encoding UTF8 resources/evaluation/ocr-synth-oof-candidate.lock.json|ConvertFrom-Json
    foreach($r in $lock.rows){foreach($pair in @(@('fusion-development.json',$r.frozen_sha256),@('full.statistics.json',$r.statistics_sha256),@('full.reports.json',$r.report_sha256))){if((Get-FileHash "$oof/$($r.edition)/$($pair[0])" -Algorithm SHA256).Hash.ToLowerInvariant() -cne $pair[1]){throw 'Frozen candidate changed'}}}
    $net=@();if($Offline){$net+='--offline'}
    Invoke-KazeCargo -CargoArguments (@('build','--release','--locked','--example','recognition_samples')+$net)
    Invoke-KazeCargo -CargoArguments (@('build','--locked','--no-default-features','--example','fusion_baseline')+$net)
    $exe=Join-Path $repoRoot 'target/release/examples/recognition_samples.exe';$measurements=@()
    foreach($r in $lock.rows){
        $edition=$r.edition
        & target/debug/examples/fusion_baseline.exe audit-frozen "$oof/$edition/fusion-development.json" "$out/development.references.jsonl" "$oof/$edition/full.reports.json" "$oof/$edition/scope-audit.json"
        if($LASTEXITCODE -ne 0){throw 'Frozen replay failed'}
        $dictionary=if($edition -eq 'small'){'resources/sudachi/system.dic'}else{"target/sudachi-dictionaries/$edition/system.dic"}
        for($i=0;$i -lt $Repetitions;$i++){
            $stem="$dest/$edition-$i";$timing="$stem.timing.json";$report="$stem.reports.json"
            # All arguments below are fixed repo-relative paths without spaces; no shell expression construction.
            $arguments=@("$out/development.inputs.jsonl",$report,"$oof/$edition/full.statistics.json",$r.statistics_sha256,'ja.ocr.synth.v1','--source-rule',"$out/source-mapping.json",'--dictionary',$dictionary,'--timings',$timing)
            $watch=[Diagnostics.Stopwatch]::StartNew()
            $process=Start-Process -FilePath $exe -ArgumentList $arguments -WorkingDirectory $repoRoot -WindowStyle Hidden -PassThru -RedirectStandardOutput "$stem.stdout.log" -RedirectStandardError "$stem.stderr.log"
            $retainedHandle=$process.Handle
            $observedPeak=0L;$observedThreads=0;$samples=0
            while(-not $process.HasExited){
                try{$process.Refresh();$observedPeak=[Math]::Max($observedPeak,$process.PeakWorkingSet64);$observedThreads=[Math]::Max($observedThreads,@($process.Threads | Where-Object { $null -ne $_ }).Count);$samples++}catch [InvalidOperationException]{}
                Start-Sleep -Milliseconds 5
            }
            $process.WaitForExit();$watch.Stop();$process.Refresh()
            if($process.ExitCode -ne 0){throw "CPU observation failed: $stem"}
            $peak=if($observedPeak -gt 0){$observedPeak}else{$null};$cpu=$process.TotalProcessorTime.TotalMilliseconds
            $process.Dispose()
            if((Get-FileHash $report -Algorithm SHA256).Hash.ToLowerInvariant() -cne $r.report_sha256){throw 'Timed release report differs from frozen report'}
            $t=Get-Content -Raw -Encoding UTF8 $timing|ConvertFrom-Json
            if($t.build -cne 'release' -or $t.calls.Count -ne 100 -or $t.slm_calls -ne 0){throw 'Invalid timing run'}
            $measurements+=[ordered]@{edition=$edition;repetition=$i;wall_ms=$watch.Elapsed.TotalMilliseconds;process_cpu_ms=$cpu;observed_peak_working_set_bytes=$peak;observed_max_threads=$observedThreads;memory_samples=$samples;dictionary_sha256=(Get-FileHash $dictionary -Algorithm SHA256).Hash.ToLowerInvariant();timings=$t}
        }
    }
    $result=[ordered]@{schema_version='kzn.recognition.cpu_observation.v1';quality_accepted=$false;logical_processors=[Environment]::ProcessorCount;processor=$env:PROCESSOR_IDENTIFIER;os=[Environment]::OSVersion.VersionString;executable_sha256=(Get-FileHash $exe -Algorithm SHA256).Hash.ToLowerInvariant();rustc=(& (Join-Path (Split-Path (Get-KazeCargo)) 'rustc.exe') --version);commit=(git rev-parse HEAD);dirty=[bool](git status --porcelain);scope='fresh processes, filesystem cache uncontrolled; sequential 100 segments; wall includes startup and JSON I/O; live sampled peak working set (5 ms polling) may miss final peak and is not isolated library RSS; wall includes observer polling; no logistic scoring';runs=$measurements}
    [IO.File]::WriteAllText((Join-Path $repoRoot "$dest/measurements.json"),($result|ConvertTo-Json -Depth 12),(New-Object Text.UTF8Encoding($false)))
    function Get-NearestRank($values,[double]$quantile){$sorted=@($values|Sort-Object);if($sorted.Count -eq 0){return $null};return $sorted[[Math]::Max(0,[int][Math]::Ceiling($quantile*$sorted.Count)-1)]}
    $summary=@()
    foreach($edition in @('small','core','full')){
        $runs=@($measurements|Where-Object{$_.edition -eq $edition})
        $warm=@($runs|ForEach-Object{$_.timings.calls|Select-Object -Skip 1}|ForEach-Object{$_.elapsed_ms})
        $summary+=[ordered]@{edition=$edition;fresh_process_runs=$runs.Count;wall_median_ms=(Get-NearestRank @($runs.wall_ms) 0.5);cpu_median_ms=(Get-NearestRank @($runs.process_cpu_ms) 0.5);load_median_ms=(Get-NearestRank @($runs.timings.load_ms) 0.5);observed_peak_working_set_bytes=($runs.observed_peak_working_set_bytes|Measure-Object -Maximum).Maximum;observed_max_threads=($runs.observed_max_threads|Measure-Object -Maximum).Maximum;warm_calls=$warm.Count;warm_p50_ms=(Get-NearestRank $warm 0.5);warm_p95_ms=(Get-NearestRank $warm 0.95)}
    }
    $aggregate=[ordered]@{schema_version='kzn.recognition.cpu_summary.v1';quality_accepted=$false;quantile='nearest rank; first call of each fresh process excluded from warm samples';measurements_sha256=(Get-FileHash "$dest/measurements.json" -Algorithm SHA256).Hash.ToLowerInvariant();rows=$summary}
    [IO.File]::WriteAllText((Join-Path $repoRoot "$dest/summary.json"),($aggregate|ConvertTo-Json -Depth 8),(New-Object Text.UTF8Encoding($false)))
    Write-Host 'Frozen scope audit and release CPU observation complete; no fitting or test inference.'
}finally{Pop-Location}