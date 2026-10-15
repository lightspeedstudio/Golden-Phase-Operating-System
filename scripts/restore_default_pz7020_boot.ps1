# Restore default PZ7020 Golden Phase BOOT.BIN (no harness test)
#
# Default = sealed feature-completion image (g1p / VM / PhaseFS / GEM),
# NOT the G64 research sidecar image.
#
# Usage:
#   powershell -File scripts/restore_default_pz7020_boot.ps1
#   powershell -File scripts/restore_default_pz7020_boot.ps1 -DriveLetter H
#
# Insert the production microSD first. This script only copies BOOT.BIN.

param(
    [ValidatePattern('^[A-Za-z]$')]
    [string]$DriveLetter = "H"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$src = Join-Path $root "fpga\zynq\research\g64_phase_fabric_sidecar\RESTORE_DEFAULT_BOOT\BOOT.BIN"
if (-not (Test-Path $src)) {
    $src = Join-Path $root "fpga\zynq\evidence\kairos_zynq_boot\BOOT_PZ7020_FEATURE_TEST\BOOT_PZ7020_FEATURE_TEST.BIN"
}
if (-not (Test-Path $src)) {
    throw "Default BOOT source not found: $src"
}

$expected = "E17A3AC5B16DEAC3688BD93CDE6048F583B680EBC6B8099EDC701682D1E8344B"
$actual = (Get-FileHash $src -Algorithm SHA256).Hash.ToUpperInvariant()
if ($actual -ne $expected) {
    throw "SHA mismatch for default BOOT. expected=$expected actual=$actual"
}

$destRoot = "${DriveLetter}:\"
if (-not (Test-Path $destRoot)) {
    throw "Drive ${DriveLetter}: not mounted. Insert the microSD and re-run."
}
$vol = Get-Volume -DriveLetter $DriveLetter -ErrorAction SilentlyContinue
if ($null -eq $vol -or $vol.Size -eq 0) {
    throw "Drive ${DriveLetter}: has no media (Size=0). Insert/reseat the microSD."
}

$dest = Join-Path $destRoot "BOOT.BIN"
Write-Host "Source: $src"
Write-Host "SHA256: $actual"
Write-Host "Dest:   $dest"
Write-Host "Volume: label=$($vol.FileSystemLabel) fs=$($vol.FileSystem) sizeMB=$([math]::Round($vol.Size/1MB))"

Copy-Item -Force $src $dest
$destSha = (Get-FileHash $dest -Algorithm SHA256).Hash.ToUpperInvariant()
if ($destSha -ne $expected) {
    throw "Copy verify failed. destSha=$destSha"
}

Write-Host "RESTORE_OK default BOOT.BIN written to ${DriveLetter}:\BOOT.BIN"
Write-Host "No harness was run. Power-cycle the board when ready."
