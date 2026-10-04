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
    Invoke-KazeCargo -CargoArguments @('fmt', '--all', '--', '--check')
    Invoke-KazeCargo -CargoArguments (@('check', '--locked', '--lib') + $networkArgs)
    Invoke-KazeCargo -CargoArguments (@('check', '--locked', '--no-default-features', '--lib') + $networkArgs)
    Invoke-KazeCargo -CargoArguments (@('check', '--locked', '-p', 'kaze_nhanh_legacy', '--lib') + $networkArgs)
    Invoke-KazeCargo -CargoArguments (@('check', '--locked', '--no-default-features', '--features', 'qwen', '--lib') + $networkArgs)
    foreach ($mode in @('core', 'default', 'minimal', 'qwen')) {
        $treeArgs = @('tree', '--locked', '--edges', 'normal') + $networkArgs
        if ($mode -eq 'core') { $treeArgs += @('-p', 'kaze_nhanh_core') }
        if ($mode -eq 'minimal') { $treeArgs += '--no-default-features' }
        if ($mode -eq 'qwen') { $treeArgs += @('--no-default-features', '--features', 'qwen') }
        $tree = & (Get-KazeCargo) @treeArgs
        if ($LASTEXITCODE -ne 0) { throw "Cannot inspect $mode dependencies." }
        $forbidden = '(git2|pulldown-cmark|candle-core|candle-nn|candle-transformers|saku|tokenizers) v'
        if ($mode -ne 'default') { $forbidden = '(git2|pulldown-cmark|candle-core|candle-nn|candle-transformers|saku|tokenizers|sudachi|kaze_nhanh_sudachi) v' }
        if ($mode -eq 'qwen') { $forbidden = '(git2|pulldown-cmark|saku|sudachi|kaze_nhanh_legacy) v' }
        if (($tree -join "`n") -match $forbidden) { throw "Unexpected backend/legacy dependency in $mode." }
    }
    Invoke-KazeCargo -CargoArguments (@('test', '--locked', '-p', 'kaze_nhanh_core') + $networkArgs)
    Invoke-KazeCargo -CargoArguments (@('test', '--locked', '--no-default-features', '--test', 'evaluation_contracts') + $networkArgs)
    Invoke-KazeCargo -CargoArguments (@('test', '--locked', '--no-default-features', '--test', 'recognition_contracts') + $networkArgs)
    Invoke-KazeCargo -CargoArguments (@('test', '--locked', '--no-default-features', '--test', 'recognition_source_evidence') + $networkArgs)
    Invoke-KazeCargo -CargoArguments (@('test', '--locked', '--test', 'evaluation_nlp') + $networkArgs)
    & (Join-Path $PSScriptRoot 'test-sudachi-setup.ps1')
    & (Join-Path $PSScriptRoot 'verify-statistics.ps1') -Offline:$Offline
    Invoke-KazeCargo -CargoArguments (@('run', '--locked', '--example', 'candidate_baseline') + $networkArgs + @('--',
        'resources/sudachi/system.dic', 'evaluation/candidate-review-contract.jsonl', 'target/candidate-review-contract.json'))
    Invoke-KazeCargo -CargoArguments (@('test', '--locked', '--no-default-features', '--example', 'recognition_quality') + $networkArgs)
    Invoke-KazeCargo -CargoArguments (@('test', '--locked', '--no-default-features', '--example', 'validate_recognition_dataset') + $networkArgs)
    Invoke-KazeCargo -CargoArguments (@('run', '--locked', '--no-default-features', '--example', 'validate_recognition_dataset') + $networkArgs + @('--',
        'tests/fixtures/dataset/synthetic.manifest.json', 'tests/fixtures/dataset/synthetic.jsonl', 'target/dataset-contract-audit.json'))
    & (Join-Path $PSScriptRoot 'test-asr-sample-setup.ps1')
    # Production legacy runtime stays real; only fixtures use explicit mocks.
    Invoke-KazeCargo -CargoArguments (@('test', '--locked', '-p', 'kaze_nhanh_legacy', '--lib') + $networkArgs)
    Invoke-KazeCargo -CargoArguments (@('test', '--locked', '--features', 'legacy', '--test', 'nlp_resources') + $networkArgs)
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
    Invoke-KazeCargo -CargoArguments (@('test', '--locked', '--workspace', '--all-features') + $networkArgs)
} finally { Pop-Location }
