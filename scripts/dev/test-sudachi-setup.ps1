#Requires -Version 5.1
# Isolated failure-path checks; never modifies installed development dictionaries.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$fixture = Join-Path $repoRoot ('target/sudachi-setup-tests/' + [guid]::NewGuid().ToString())
foreach ($directory in @('scripts/dev','resources/sudachi','target/dev-assets')) {
    New-Item -ItemType Directory -Force -Path (Join-Path $fixture $directory) | Out-Null
}
$script = Join-Path $fixture 'scripts/dev/setup-sudachi.ps1'
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'setup-sudachi.ps1') -Destination $script
$manifest = [ordered]@{ package = 'fixture-full'; version = 'test'; archive_name = 'fixture.zip'
    url = 'https://example.invalid/never-downloaded'; sha256 = ('0' * 64)
    dictionary_entry = 'dictionary/system_full.dic'; dictionary_sha256 = ('0' * 64) }
$lock = Join-Path $fixture 'resources/sudachi/dictionary.full.lock.json'
$cache = Join-Path $fixture 'target/dev-assets/fixture.zip'
function Save-Lock { [IO.File]::WriteAllText($lock, ($manifest | ConvertTo-Json), (New-Object Text.UTF8Encoding($false))) }
function Assert-Rejected([string]$message) {
    $rejected = $false
    try { & $script -Offline -Edition full } catch {
        if ($_.Exception.Message -notlike "*$message*") { throw }
        $rejected = $true
    }
    if (-not $rejected) { throw "Expected refusal: $message" }
}
Save-Lock
Assert-Rejected 'archive missing'
[IO.File]::WriteAllText($cache,'invalid-cache')
Assert-Rejected 'Cached archive SHA256 mismatch'
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
Remove-Item -LiteralPath $cache
$zip = [IO.Compression.ZipFile]::Open($cache, [IO.Compression.ZipArchiveMode]::Create)
try {
    foreach ($entryName in @('dictionary/system_full.dic','dictionary/LEGAL')) {
        $entry=$zip.CreateEntry($entryName)
        $writer=New-Object IO.StreamWriter($entry.Open())
        try { $writer.Write('synthetic-fixture') } finally { $writer.Dispose() }
    }
} finally { $zip.Dispose() }
$manifest.sha256=(Get-FileHash -LiteralPath $cache -Algorithm SHA256).Hash.ToLowerInvariant()
Save-Lock
Assert-Rejected 'Extracted dictionary SHA256 mismatch'
$destination=Join-Path $fixture 'target/sudachi-dictionaries/full/system.dic'
if (Test-Path -LiteralPath $destination) { throw 'Rejected dictionary was installed.' }
$staging=Join-Path $fixture 'target/sudachi-dictionaries/full/system.dic.part'
$manifest.dictionary_sha256=(Get-FileHash -LiteralPath $staging -Algorithm SHA256).Hash.ToLowerInvariant()
Save-Lock
& $script -Offline -Edition full
if (-not (Test-Path -LiteralPath $destination) -or -not (Test-Path -LiteralPath (Join-Path (Split-Path $destination) 'licenses/LEGAL'))) {
    throw 'Verified ZIP dictionary and LEGAL were not installed.'
}
Write-Host 'Sudachi edition setup failure paths and ZIP/LEGAL extraction passed.'
