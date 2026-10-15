# Safe full test run: limit cargo parallelism and test threads.
# Usage: .\scripts\test-safe.ps1 [-LogPath <file>]
param(
    [string]$LogPath = ""
)

$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root

$env:CARGO_BUILD_JOBS = "2"
$env:CARGO_INCREMENTAL = "0"

if (-not $LogPath) {
    $LogPath = Join-Path $root "release_snapshot\logs\test_output_safe.txt"
}
$logDir = Split-Path $LogPath -Parent
if ($logDir) { New-Item -ItemType Directory -Force -Path $logDir | Out-Null }

Write-Host "Running cargo test with CARGO_BUILD_JOBS=2, --test-threads=2"
Write-Host "Log: $LogPath"
Write-Warning "Do not run export-benchmarks-safe.ps1 -ForceReRun or baremetal release builds at the same time."

& cargo test --jobs 2 -- --test-threads=2 2>&1 | Tee-Object -FilePath $LogPath
exit $LASTEXITCODE
