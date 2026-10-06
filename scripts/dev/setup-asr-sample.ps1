#Requires -Version 5.1
[CmdletBinding()]
param([switch]$Offline)
. (Join-Path $PSScriptRoot 'common.ps1')
$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$lock = Get-Content -LiteralPath (Join-Path $repoRoot 'resources/evaluation/asr-ja-evalkit.lock.json') -Raw -Encoding UTF8 | ConvertFrom-Json
if ($lock.schema_version -ne 'kzn.public_sample.lock.v1' -or $lock.revision -cnotmatch '^[a-f0-9]{40}$') { throw 'Invalid sample lock.' }
$outputDirectory = Join-Path $repoRoot 'target/asr-ja-evalkit-sample'
New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null
foreach ($file in $lock.files) {
    if ($file.name -cnotmatch '^[A-Za-z0-9_.-]+$' -or $file.name -in @('.', '..') -or
        $file.sha256 -cnotmatch '^[a-f0-9]{64}$' -or $file.bytes -le 0 -or $file.bytes -gt 1048576 -or
        -not $file.url.StartsWith("https://raw.githubusercontent.com/ouktlab/asr-ja_evalkit/$($lock.revision)/", [StringComparison]::Ordinal)) { throw 'Invalid locked sample file.' }
    $destination = Join-Path $outputDirectory $file.name
    if (-not (Test-Path -LiteralPath $destination)) {
        if ($Offline) { throw "Missing cached sample: $($file.name)" }
        $temporary = "$destination.part"
        try {
            Invoke-WebRequest -UseBasicParsing -Uri $file.url -OutFile $temporary
            if ((Get-Item -LiteralPath $temporary).Length -ne $file.bytes -or
                (Get-FileHash -LiteralPath $temporary -Algorithm SHA256).Hash.ToLowerInvariant() -cne $file.sha256) { throw "Downloaded sample integrity failure: $($file.name)" }
            Move-Item -LiteralPath $temporary -Destination $destination
        } finally {
            if (Test-Path -LiteralPath $temporary) { Remove-Item -LiteralPath $temporary }
        }
    }
    if ((Get-Item -LiteralPath $destination).Length -ne $file.bytes -or
        (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash.ToLowerInvariant() -cne $file.sha256) { throw "Cached sample integrity failure: $($file.name)" }
}
Write-Host "Verified illustrative sample at $outputDirectory; not a verified ASR measurement."