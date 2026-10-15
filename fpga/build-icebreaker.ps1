# Build + program full physical suite (gates + GPL VM)
param(
    [string]$EvidenceName = "complex_gates"
)
$ErrorActionPreference = "Stop"
$CAD = "C:\Tools\oss-cad-suite\oss-cad-suite"
if (-not (Test-Path "$CAD\bin\yosys.exe")) {
    throw "OSS CAD Suite not found at $CAD"
}
$env:PATH = "$CAD\bin;$CAD\lib;$env:PATH"
Set-Location $PSScriptRoot

$evidence = Join-Path $PSScriptRoot "evidence\$EvidenceName"
New-Item -ItemType Directory -Force -Path $evidence | Out-Null

$srcs = @(
    "phase_alu.v", "phase_registers.v", "phase_decoder.v",
    "topology_unit.v", "trace_bus.v", "phase_cpu.v",
    "uart_tx.v", "physical_gates_uart.v", "complex_gates_uart.v",
    "gpl_vm_uart.v", "physical_suite_uart.v", "icebreaker_top.v"
)

Write-Host "== yosys synth_ice40 =="
$read = ($srcs | ForEach-Object { $_ }) -join " "
& yosys -l (Join-Path $evidence "yosys.log") -p "read_verilog $read; synth_ice40 -top icebreaker_top -json icebreaker.json"
if ($LASTEXITCODE -ne 0) { throw "yosys failed" }

Write-Host "== nextpnr-ice40 =="
$pnrLog = Join-Path $evidence "nextpnr.log"
$pnrReport = Join-Path $evidence "nextpnr_report.json"
cmd /c "nextpnr-ice40 --up5k --package sg48 --json icebreaker.json --pcf icebreaker.pcf --asc icebreaker.asc --report `"$pnrReport`" > `"$pnrLog`" 2>&1"
if ($LASTEXITCODE -ne 0) { Get-Content $pnrLog -Tail 40; throw "nextpnr failed" }

Write-Host "== icepack =="
& icepack icebreaker.asc icebreaker.bin
if ($LASTEXITCODE -ne 0) { throw "icepack failed" }

Copy-Item icebreaker.bin (Join-Path $evidence "icebreaker.bin") -Force
Copy-Item icebreaker.asc (Join-Path $evidence "icebreaker.asc") -Force
$hash = (Get-FileHash -Algorithm SHA256 icebreaker.bin).Hash.ToLower()
Set-Content -Path (Join-Path $evidence "icebreaker.bin.sha256") -Value "$hash  icebreaker.bin" -NoNewline
Write-Host "BITSTREAM SHA256: $hash"

Write-Host "== iceprog =="
& iceprog icebreaker.bin
if ($LASTEXITCODE -ne 0) { throw "iceprog failed - unplug/replug USB and retry" }

Write-Host "FLASH OK"
