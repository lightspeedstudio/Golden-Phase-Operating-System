# Build phase lattice causal-curvature bitstream (no flash — see flash-capture-phase-lattice.ps1)
$ErrorActionPreference = "Stop"
$CAD = "C:\Tools\oss-cad-suite\oss-cad-suite"
if (-not (Test-Path "$CAD\bin\yosys.exe")) {
    throw "OSS CAD Suite not found at $CAD"
}
$env:PATH = "$CAD\bin;$CAD\lib;$env:PATH"
Set-Location $PSScriptRoot

$evidence = Join-Path $PSScriptRoot "evidence\phase_lattice"
New-Item -ItemType Directory -Force -Path $evidence | Out-Null

$srcs = @(
    "uart_tx.v", "phase_lattice_uart.v", "phase_lattice_top.v"
)

Write-Host "== yosys synth_ice40 =="
$read = ($srcs | ForEach-Object { $_ }) -join " "
& yosys -l (Join-Path $evidence "yosys.log") -p "read_verilog $read; synth_ice40 -top phase_lattice_top -json phase_lattice.json"
if ($LASTEXITCODE -ne 0) { throw "yosys failed" }

Write-Host "== nextpnr-ice40 =="
$pnrLog = Join-Path $evidence "nextpnr.log"
$pnrReport = Join-Path $evidence "nextpnr_report.json"
cmd /c "nextpnr-ice40 --up5k --package sg48 --json phase_lattice.json --pcf icebreaker.pcf --asc phase_lattice.asc --report `"$pnrReport`" > `"$pnrLog`" 2>&1"
if ($LASTEXITCODE -ne 0) { Get-Content $pnrLog -Tail 40; throw "nextpnr failed" }

Write-Host "== icepack =="
& icepack phase_lattice.asc phase_lattice.bin
if ($LASTEXITCODE -ne 0) { throw "icepack failed" }

Copy-Item phase_lattice.asc (Join-Path $evidence "icebreaker.asc") -Force
$hash = (Get-FileHash -Algorithm SHA256 phase_lattice.bin).Hash.ToLower()
Write-Host "BITSTREAM SHA256: $hash"

Get-Content $pnrLog | Select-String -Pattern "Device utilisation|ICESTORM_LC|ICESTORM_RAM|SB_IO|SB_GB|Max frequency" | ForEach-Object { Write-Host $_.Line }
Write-Host "BUILD OK: phase_lattice.bin (flash with flash-capture-phase-lattice.ps1)"
