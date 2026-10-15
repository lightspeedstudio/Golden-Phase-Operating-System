# KairOS QEMU RISC-V virt boot
$ErrorActionPreference = "Stop"
Set-Location (Split-Path $PSScriptRoot -Parent)
$cargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
$elf = Join-Path $PWD "target\riscv64gc-unknown-none-elf\debug\kairos_boot"
$previousRustFlags = $env:RUSTFLAGS

$env:Path = [System.Environment]::GetEnvironmentVariable("Path", "Machine") + ";" +
    [System.Environment]::GetEnvironmentVariable("Path", "User")
$env:RUSTFLAGS = "$previousRustFlags -Awarnings".Trim()

$qemu = (Get-Command qemu-system-riscv64 -ErrorAction SilentlyContinue).Source
if (-not $qemu) {
    $candidates = @(
        "C:\Program Files\qemu\qemu-system-riscv64.exe",
        "C:\Program Files\QEMU\qemu-system-riscv64.exe",
        "C:\ProgramData\chocolatey\bin\qemu-system-riscv64.exe"
    )
    foreach ($c in $candidates) {
        if (Test-Path $c) { $qemu = $c; break }
    }
}

& $cargo build --bin kairos_boot --no-default-features --features bm-full --target riscv64gc-unknown-none-elf
if (-not (Test-Path $elf)) {
    throw "missing boot image: $elf"
}
if (-not $qemu) {
    Write-Host "QEMU not found. Close and reopen PowerShell after admin install, then re-run."
    Write-Host "Manual: choco install qemu -y  (elevated PowerShell)"
    exit 1
}

Write-Host "Booting $elf"
Write-Host "QEMU: $qemu"
Write-Host "Quit: Ctrl+A then X"
try {
    & $qemu -machine virt -m 128M -display none -monitor none -bios none -kernel $elf -serial stdio
}
finally {
    $env:RUSTFLAGS = $previousRustFlags
}
