# Build homogeneous vacuum/lock lattice bitstream
param(
    [ValidateSet(0, 3)]
    [int]$Phi = 0
)
$ErrorActionPreference = "Stop"
$CAD = "C:\Tools\oss-cad-suite\oss-cad-suite"
if (-not (Test-Path "$CAD\bin\yosys.exe")) { throw "OSS CAD Suite not found at $CAD" }
$env:PATH = "$CAD\bin;$CAD\lib;$env:PATH"
Set-Location $PSScriptRoot

$tag = if ($Phi -eq 0) { "phi0" } else { "phi3" }
$evidence = Join-Path $PSScriptRoot "evidence\phase_lattice_$tag"
New-Item -ItemType Directory -Force -Path $evidence | Out-Null
$bin = "phase_lattice_$tag.bin"
$json = "phase_lattice_$tag.json"
$asc = "phase_lattice_$tag.asc"

Write-Host "== yosys synth_ice40 (vacuum Phi$Phi) =="
& yosys -l (Join-Path $evidence "yosys.log") -p "read_verilog uart_tx.v phase_lattice_uart.v phase_lattice_vac.v; chparam -set PHI $Phi phase_lattice_vac_top; synth_ice40 -top phase_lattice_vac_top -json $json"
if ($LASTEXITCODE -ne 0) { throw "yosys failed" }

Write-Host "== nextpnr-ice40 =="
$pnrLog = Join-Path $evidence "nextpnr.log"
$pnrReport = Join-Path $evidence "nextpnr_report.json"
cmd /c "nextpnr-ice40 --up5k --package sg48 --json $json --pcf icebreaker.pcf --asc $asc --report `"$pnrReport`" > `"$pnrLog`" 2>&1"
if ($LASTEXITCODE -ne 0) { Get-Content $pnrLog -Tail 40; throw "nextpnr failed" }

Write-Host "== icepack =="
& icepack $asc $bin
if ($LASTEXITCODE -ne 0) { throw "icepack failed" }

Copy-Item $bin (Join-Path $evidence "icebreaker.bin") -Force
Copy-Item $asc (Join-Path $evidence "icebreaker.asc") -Force
$hash = (Get-FileHash -Algorithm SHA256 $bin).Hash.ToLower()
Set-Content (Join-Path $evidence "icebreaker.bin.sha256") "$hash  icebreaker.bin" -NoNewline
Write-Host "BUILD OK: $bin SHA256=$hash"
