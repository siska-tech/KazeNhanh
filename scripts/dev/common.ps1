#Requires -Version 5.1
# Shared helpers. No permanent PATH or machine configuration changes.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-KazeCargo {
    $command = Get-Command cargo -ErrorAction SilentlyContinue
    if ($command) { return $command.Source }
    $cargoRoot = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $HOME '.cargo' }
    $binary = if ($env:OS -eq 'Windows_NT') { 'bin/cargo.exe' } else { 'bin/cargo' }
    $candidate = Join-Path $cargoRoot $binary
    if (Test-Path -LiteralPath $candidate) { return $candidate }
    throw 'Cargo not found. Install Rust with rustup (https://rustup.rs), then run setup-dev.ps1 again.'
}

function Invoke-KazeCargo {
    param([Parameter(Mandatory)][string[]]$CargoArguments)
    & (Get-KazeCargo) @CargoArguments
    if ($LASTEXITCODE -ne 0) {
        throw "cargo $($CargoArguments -join ' ') failed (exit $LASTEXITCODE)."
    }
}
