#Requires -Version 5.1
[CmdletBinding()]
param([switch]$Offline, [ValidateSet('small', 'core', 'full')][string]$Edition = 'small')
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$assetDir = Join-Path $repoRoot 'resources/sudachi'
$lockName = if ($Edition -eq 'small') { 'dictionary.lock.json' } else { "dictionary.$Edition.lock.json" }
$package = Get-Content -LiteralPath (Join-Path $assetDir $lockName) -Raw -Encoding UTF8 | ConvertFrom-Json
if ($Edition -ne 'small') { $assetDir = Join-Path $repoRoot "target/sudachi-dictionaries/$Edition" }
New-Item -ItemType Directory -Path $assetDir -Force | Out-Null
$cacheDir = Join-Path $repoRoot 'target/dev-assets'
New-Item -ItemType Directory -Path $cacheDir -Force | Out-Null
$archiveName = if ($package.PSObject.Properties['archive_name']) { $package.archive_name } else { "$($package.package)-$($package.version).whl" }
$archive = Join-Path $cacheDir $archiveName

if (-not (Test-Path -LiteralPath $archive)) {
    if ($Offline) { throw "Dictionary archive missing: $archive. Run setup-sudachi.ps1 -Edition $Edition online once." }
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    $partial = "$archive.part"
    Write-Host "Downloading $($package.package) $($package.version)..."
    $previousProgress = $ProgressPreference
    try {
        $ProgressPreference = 'SilentlyContinue'
        Invoke-WebRequest -Uri $package.url -OutFile $partial -UseBasicParsing
    } finally { $ProgressPreference = $previousProgress }
    if ((Get-FileHash -LiteralPath $partial -Algorithm SHA256).Hash -ne $package.sha256) {
        throw "Dictionary SHA256 mismatch: $partial. Archive was not installed."
    }
    Move-Item -LiteralPath $partial -Destination $archive -Force
}
if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ne $package.sha256) {
    throw "Cached archive SHA256 mismatch: $archive. Remove this file and retry."
}

# Wheel and official distribution are ZIP archives. Only extract the named dictionary and license files;
# never execute Python/package code or use archive paths as output paths.
Add-Type -AssemblyName System.IO.Compression.FileSystem
$zip = [IO.Compression.ZipFile]::OpenRead($archive)
try {
    $entry = $zip.GetEntry($package.dictionary_entry)
    if (-not $entry) { throw "Dictionary entry missing: $($package.dictionary_entry)" }
    $destination = Join-Path $assetDir 'system.dic'
    $staging = Join-Path $assetDir 'system.dic.part'
    [IO.Compression.ZipFileExtensions]::ExtractToFile($entry, $staging, $true)
    if ($package.PSObject.Properties['dictionary_sha256'] -and
        (Get-FileHash -LiteralPath $staging -Algorithm SHA256).Hash -ne $package.dictionary_sha256) {
        throw 'Extracted dictionary SHA256 mismatch. Dictionary was not installed.'
    }
    $licenses = @($zip.Entries | Where-Object { $_.Name -match '^(LICENSE|NOTICE|COPYING|LEGAL)' })
    if ($licenses.Count -eq 0) { throw 'Dictionary package has no license files; review the package.' }
    $licenseDir = Join-Path $assetDir 'licenses'
    New-Item -ItemType Directory -Path $licenseDir -Force | Out-Null
    foreach ($license in $licenses) {
        [IO.Compression.ZipFileExtensions]::ExtractToFile($license, (Join-Path $licenseDir $license.Name), $true)
    }
    Move-Item -LiteralPath $staging -Destination $destination -Force
} finally { $zip.Dispose() }
Write-Host "Sudachi dictionary ready: $destination"
Write-Host "SHA256: $((Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash)"
Write-Host "Licenses: $licenseDir"
