# Build parameterized Avenue 2 glider bitstream.
param(
    [ValidateSet(4, 8)]
    [int]$M = 4,
    [int]$GENS = 8,
    [int]$VACUUM = 1,
    [int]$DEFECT = 2,
    [int]$SEED_R = -1,
    [int]$SEED_C = -1,
    [string]$OutName = ""
)
$ErrorActionPreference = "Stop"
$CAD = "C:\Tools\oss-cad-suite\oss-cad-suite"
$YOSYS = Join-Path $CAD "bin\yosys.exe"
$NEXTPNR = Join-Path $CAD "bin\nextpnr-ice40.exe"
$ICEPACK = Join-Path $CAD "bin\icepack.exe"
if (-not (Test-Path $YOSYS)) { throw "OSS CAD Suite not found at $CAD" }
$env:PATH = "$CAD\bin;$CAD\lib;$env:PATH"
Set-Location $PSScriptRoot

if ($SEED_R -lt 0) { $SEED_R = [int]($M / 2) }
if ($SEED_C -lt 0) { $SEED_C = [int]($M / 2) }
if ([string]::IsNullOrEmpty($OutName)) {
    $OutName = "phase_lattice_2d_glider_M${M}_G${GENS}_V${VACUUM}D${DEFECT}_S${SEED_R}${SEED_C}"
}

$ev = Join-Path $PSScriptRoot "evidence\$OutName"
New-Item -ItemType Directory -Force -Path $ev | Out-Null
$json = "$OutName.json"
$asc = "$OutName.asc"
$bin = "$OutName.bin"

Write-Host "== yosys glider M=$M GENS=$GENS V=$VACUUM D=$DEFECT S=$SEED_R,$SEED_C =="
$ys = @"
read_verilog uart_tx.v phase_lattice_uart.v phase_lattice_2d_glider.v
chparam -set M $M phase_lattice_2d_glider_top
chparam -set GENS $GENS phase_lattice_2d_glider_top
chparam -set VACUUM $VACUUM phase_lattice_2d_glider_top
chparam -set DEFECT $DEFECT phase_lattice_2d_glider_top
chparam -set SEED_R $SEED_R phase_lattice_2d_glider_top
chparam -set SEED_C $SEED_C phase_lattice_2d_glider_top
synth_ice40 -top phase_lattice_2d_glider_top -json $json
"@
& $YOSYS -l (Join-Path $ev "yosys.log") -p $ys
if ($LASTEXITCODE -ne 0) { throw "yosys failed" }

Write-Host "== nextpnr-ice40 =="
$pnrLog = Join-Path $ev "nextpnr.log"
$pnrReport = Join-Path $ev "nextpnr_report.json"
# nextpnr writes Info to stderr; with $ErrorActionPreference=Stop that becomes a terminating error.
cmd /c "`"$NEXTPNR`" --up5k --package sg48 --json `"$json`" --pcf icebreaker.pcf --asc `"$asc`" --report `"$pnrReport`" > `"$pnrLog`" 2>&1"
if ($LASTEXITCODE -ne 0) { Get-Content $pnrLog -Tail 40; throw "nextpnr failed" }

& $ICEPACK $asc $bin
if ($LASTEXITCODE -ne 0) { throw "icepack failed" }

$hash = (Get-FileHash -Algorithm SHA256 $bin).Hash.ToLower()
Copy-Item $bin (Join-Path $ev "icebreaker.bin") -Force
Set-Content (Join-Path $ev "icebreaker.bin.sha256") "$hash  icebreaker.bin" -NoNewline
Get-Content $pnrLog | Select-String -Pattern "ICESTORM_LC|Max frequency" | ForEach-Object { Write-Host $_.Line }
Write-Host "BITSTREAM SHA256: $hash"
Write-Host "BUILD OK: $bin"
