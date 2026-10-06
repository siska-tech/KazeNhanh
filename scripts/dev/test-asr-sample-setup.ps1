#Requires -Version 5.1
. (Join-Path $PSScriptRoot 'common.ps1')
$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$fixtureRoot = Join-Path $repoRoot ('target/asr-sample-contract-' + [Guid]::NewGuid().ToString('N'))
$scripts = Join-Path $fixtureRoot 'scripts/dev'
$resources = Join-Path $fixtureRoot 'resources/evaluation'
$cache = Join-Path $fixtureRoot 'target/asr-ja-evalkit-sample'
New-Item -ItemType Directory -Force $scripts,$resources,$cache | Out-Null
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'setup-asr-sample.ps1'),(Join-Path $PSScriptRoot 'common.ps1') -Destination $scripts
$sample = Join-Path $cache 'sample.txt'
[IO.File]::WriteAllText($sample, 'synthetic setup contract', (New-Object Text.UTF8Encoding($false)))
$revision = 'a' * 40
$lock = @{schema_version='kzn.public_sample.lock.v1';revision=$revision;files=@(@{
    name='sample.txt';url="https://raw.githubusercontent.com/ouktlab/asr-ja_evalkit/$revision/sample.txt"
    sha256=(Get-FileHash -LiteralPath $sample -Algorithm SHA256).Hash.ToLowerInvariant();bytes=(Get-Item -LiteralPath $sample).Length
})}
$lockPath = Join-Path $resources 'asr-ja-evalkit.lock.json'
function Save-Lock { [IO.File]::WriteAllText($lockPath, ($lock | ConvertTo-Json -Depth 5), (New-Object Text.UTF8Encoding($false))) }
function Expect-Failure([string]$reason) {
    $failed = $false
    try { & (Join-Path $scripts 'setup-asr-sample.ps1') -Offline } catch { $failed = $true }
    if (-not $failed) { throw "Expected setup rejection: $reason" }
}
Save-Lock
& (Join-Path $scripts 'setup-asr-sample.ps1') -Offline
[IO.File]::WriteAllText($sample, 'corrupt', (New-Object Text.UTF8Encoding($false)))
Expect-Failure 'corrupt cached bytes'
$lock.files[0].name = 'missing.txt'; Save-Lock
Expect-Failure 'missing offline cache'
$lock.files[0].name = '../outside.txt'; Save-Lock
Expect-Failure 'invalid relative path'
$lock.files[0].name = 'sample.txt'; $lock.files[0].url = 'https://example.invalid/sample.txt'; Save-Lock
Expect-Failure 'unexpected source URL'
Write-Host 'ASR sample setup contracts passed (offline; synthetic data).'