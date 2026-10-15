# PN-WIRE-1A: QEMU Golden Phase virtio peer (socket L2) for PZ7020 physical wire.
#
# Classification: Physical PZ7020-to-QEMU PhaseNet peer proof (development).
# NOT a two-board PhaseNet seal (that is PN-WIRE-1B).
#
# Usage:
#   1) Start this script (QEMU listens on TCP 1234 for QEMU socket netdev).
#   2) At gpos>:  net init ; net peer 16
#   3) Bridge physical NIC <-> QEMU socket:
#        python scripts/pn_wire1a_l2_bridge.py --iface "Ethernet" --qemu 127.0.0.1:1234
#   4) On PZ7020 COM12:  net wire init ; net wire ping
#
# Quit QEMU: Ctrl+A then X

param(
    [int]$LinkPort = 1234,
    [string]$Mac = "52:54:00:47:01:a1"
)

$ErrorActionPreference = "Stop"
Set-Location (Split-Path $PSScriptRoot -Parent)
$cargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
$elf = Join-Path $PWD "target\riscv64gc-unknown-none-elf\release\kairos_boot"
$previousRustFlags = $env:RUSTFLAGS

$env:Path = [System.Environment]::GetEnvironmentVariable("Path", "Machine") + ";" +
    [System.Environment]::GetEnvironmentVariable("Path", "User")
$env:RUSTFLAGS = "$previousRustFlags -Awarnings".Trim()

$qemu = (Get-Command qemu-system-riscv64 -ErrorAction SilentlyContinue).Source
if (-not $qemu) {
    foreach ($c in @(
            "C:\Program Files\qemu\qemu-system-riscv64.exe",
            "C:\Program Files\QEMU\qemu-system-riscv64.exe",
            "C:\ProgramData\chocolatey\bin\qemu-system-riscv64.exe"
        )) {
        if (Test-Path $c) { $qemu = $c; break }
    }
}

Write-Host "Building kairos_boot (bm-embedded = baremetal + bm-net)..."
& $cargo build --release --bin kairos_boot --no-default-features --features bm-embedded --target riscv64gc-unknown-none-elf
if (-not (Test-Path $elf)) { throw "missing boot image: $elf" }
if (-not $qemu) { throw "qemu-system-riscv64 not found" }

Write-Host "============================================================"
Write-Host "PN-WIRE-1A QEMU peer  (NOT a two-board seal)"
Write-Host "ELF: $elf"
Write-Host "QEMU socket listen :$LinkPort  mac=$Mac"
Write-Host "Then: python scripts/pn_wire1a_l2_bridge.py --iface <NIC> --qemu 127.0.0.1:$LinkPort"
Write-Host "Guest: net init ; net peer 16"
Write-Host "Board: net wire init ; net wire ping"
Write-Host "============================================================"

try {
    & $qemu `
        -machine virt `
        -m 128M `
        -display none `
        -monitor none `
        -bios none `
        -kernel $elf `
        -netdev "socket,id=n0,listen=:$LinkPort" `
        -device "virtio-net-device,netdev=n0,mac=$Mac" `
        -serial stdio
}
finally {
    $env:RUSTFLAGS = $previousRustFlags
}
