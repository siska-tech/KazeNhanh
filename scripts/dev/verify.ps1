#Requires -Version 5.1
[CmdletBinding()]
param([switch]$Offline)
. (Join-Path $PSScriptRoot 'common.ps1')
$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Push-Location $repoRoot
try {
    if (-not (Test-Path -LiteralPath 'resources/sudachi/system.dic')) {
        throw 'Missing Sudachi dictionary. Run ./scripts/dev/setup-dev.ps1 first.'
    }
    $networkArgs = @()
    if ($Offline) { $networkArgs += '--offline' }
    # Independent contracts must remain free of all NLP/model/Git backends.
    $coreManifest = 'crates/core/Cargo.toml'
    Invoke-KazeCargo -CargoArguments (@('test', '--manifest-path', $coreManifest, '--target-dir', 'target', '--locked') + $networkArgs)
    $tree = & (Get-KazeCargo) tree --manifest-path $coreManifest --locked --edges normal @networkArgs
    if ($LASTEXITCODE -ne 0) { throw 'Cannot inspect core dependencies.' }
    if (($tree -join "`n") -match '(?m)\b(git2|pulldown-cmark|candle-core|candle-nn|candle-transformers|sudachi|saku) v') {
        throw 'Evaluation core must not depend on backend/legacy libraries.'
    }
    Invoke-KazeCargo -CargoArguments (@('test', '--locked', '--test', 'evaluation_contracts') + $networkArgs)
    Invoke-KazeCargo -CargoArguments @('fmt', '--all', '--', '--check')
    Invoke-KazeCargo -CargoArguments (@('check', '--locked', '--lib') + $networkArgs)
    # Unit tests compile the production backend and inject mocks explicitly.
    Invoke-KazeCargo -CargoArguments (@('test', '--locked', '--lib') + $networkArgs)
    # The public constructor always uses the production backend.
    Invoke-KazeCargo -CargoArguments (@('test', '--locked', '--test', 'nlp_resources') + $networkArgs)
    $expected = @{
        api_workflows = @('git_report_includes_markdown_additions', 'git_native_rag_synthesizes_summary_for_changes', 'summarize_with_details_returns_sentences_and_summary')
        thread_safety = @('summarize_with_details_is_thread_safe', 'git_native_rag_handles_parallel_invocations')
    }
    foreach ($target in $expected.Keys) {
        $listing = & (Get-KazeCargo) test --locked --features mock_inference --test $target @networkArgs -- --list
        if ($LASTEXITCODE -ne 0) { throw "Cannot list $target tests." }
        foreach ($name in $expected[$target]) {
            if ($listing -notcontains "${name}: test") { throw "Expected test not registered: $target/$name" }
        }
    }
    Invoke-KazeCargo -CargoArguments (@('test', '--locked', '--features', 'mock_inference') + $networkArgs)
} finally { Pop-Location }
