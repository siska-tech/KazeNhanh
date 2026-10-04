#Requires -Version 5.1
[CmdletBinding()]
param([switch]$Offline, [switch]$CheckOnly, [switch]$Verify, [switch]$AllDictionaries)
. (Join-Path $PSScriptRoot 'common.ps1')
$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Push-Location $repoRoot
try {
    Invoke-KazeCargo -CargoArguments @('--version')
    if ($env:OS -eq 'Windows_NT') {
        $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
        if (-not (Test-Path -LiteralPath $vswhere)) {
            throw 'Install Visual Studio Build Tools: Desktop development with C++, MSVC and Windows SDK. See docs/development-setup.md.'
        }
        $vsInstallation = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        if ($LASTEXITCODE -ne 0 -or -not $vsInstallation) {
            throw 'MSVC x64/x86 tools are missing. Install the C++ workload and Windows SDK.'
        }
        Write-Host "MSVC installation: $vsInstallation"
    }
    if (-not (Get-Command git -ErrorAction SilentlyContinue)) { throw 'Git is required; install Git and rerun.' }
    if ($CheckOnly) {
        Write-Host 'Prerequisite discovery passed. Native linking is verified by -Verify.'
        return
    }
    $fetchArgs = @('fetch', '--locked')
    if ($Offline) { $fetchArgs += '--offline' }
    Invoke-KazeCargo -CargoArguments $fetchArgs
    & (Join-Path $PSScriptRoot 'setup-sudachi.ps1') -Offline:$Offline
    if ($AllDictionaries) {
        foreach ($edition in @('core', 'full')) { & (Join-Path $PSScriptRoot 'setup-sudachi.ps1') -Edition $edition -Offline:$Offline }
    }
    if ($Verify) { & (Join-Path $PSScriptRoot 'verify.ps1') -Offline:$Offline }
    else { Write-Host 'Next: ./scripts/dev/verify.ps1 (run with powershell or pwsh).' }
} finally { Pop-Location }
