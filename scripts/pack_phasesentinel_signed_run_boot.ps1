# Pack Utility BOOT.BIN for PhaseSentinel-gated signed app run.
# Requires D:\Xilinx\2026.1 unlocked (bootgen + arm-none-eabi-gcc).
$ErrorActionPreference = "Stop"
$Kairos = Split-Path -Parent $PSScriptRoot
$Out = Join-Path $Kairos "fpga\zynq\evidence\kairos_zynq_boot\BOOT_PHASESENTINEL_SIGNED_RUN_20260815"
$Fsbl = Join-Path $Kairos "fpga\zynq\release\PZ7020_G1P_BRINGUP_V2\zynq_fsbl.elf"
$Bit = Join-Path $Kairos "fpga\zynq\release\PZ7020_G1P_PATH1_G64\system.bit"
$ElfSrc = Join-Path $Kairos "target\armv7a-none-eabi\release\kairos_zynq"
$Bootgen = "D:\Xilinx\2026.1\Vitis\bin\bootgen.bat"

if (-not (Test-Path $Bootgen)) { throw "bootgen missing (unlock D: BitLocker): $Bootgen" }
if (-not (Test-Path $ElfSrc)) { throw "ELF missing: $ElfSrc" }
$bitHash = (Get-FileHash -Algorithm SHA256 $Bit).Hash
if ($bitHash -ne "D19F08D20CAC85EA0E33C41A8A552166F5A9176E16977F09AA64D17E68135829") {
    throw "PATH1_G64 system.bit hash mismatch: $bitHash"
}

New-Item -ItemType Directory -Force -Path $Out | Out-Null
Copy-Item $ElfSrc (Join-Path $Out "kairos_zynq.elf") -Force
$Elf = Join-Path $Out "kairos_zynq.elf"
$Bif = Join-Path $Out "boot.bif"
@"
the_ROM_image:
{
  [bootloader] $Fsbl
  $Bit
  $Elf
}
"@ | Set-Content -Encoding ascii $Bif

& $Bootgen -image $Bif -arch zynq -w -o (Join-Path $Out "BOOT.BIN")
if ($LASTEXITCODE -ne 0) { throw "bootgen failed: $LASTEXITCODE" }

Get-FileHash -Algorithm SHA256 $Elf,(Join-Path $Out "BOOT.BIN"),$Bit | ForEach-Object {
    Write-Host "$($_.Hash)  $($_.Path)"
}
