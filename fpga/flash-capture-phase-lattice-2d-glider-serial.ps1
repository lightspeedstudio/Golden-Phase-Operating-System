# Build/flash cell-serial glider (default M=8 for timing seal).
param(
    [ValidateSet(4, 8)]
    [int]$M = 8,
    [int]$GENS = 16,
    [int]$VACUUM = 1,
    [int]$DEFECT = 2,
    [int]$SEED_R = -1,
    [int]$SEED_C = -1
)
$ErrorActionPreference = "Stop"
$CAD = "C:\Tools\oss-cad-suite\oss-cad-suite"
$YOSYS = Join-Path $CAD "bin\yosys.exe"
$NEXTPNR = Join-Path $CAD "bin\nextpnr-ice40.exe"
$ICEPACK = Join-Path $CAD "bin\icepack.exe"
$ICEPROG = Join-Path $CAD "bin\iceprog.exe"
$env:PATH = "$CAD\bin;$CAD\lib;$env:PATH"
Set-Location $PSScriptRoot
if ($SEED_R -lt 0) { $SEED_R = [int]($M / 2) }
if ($SEED_C -lt 0) { $SEED_C = [int]($M / 2) }
$OutName = "phase_lattice_2d_glider_serial_M${M}_G${GENS}_V${VACUUM}D${DEFECT}_S${SEED_R}${SEED_C}"
$ev = Join-Path $PSScriptRoot "evidence\$OutName"
New-Item -ItemType Directory -Force -Path $ev | Out-Null
$json = "$OutName.json"; $asc = "$OutName.asc"; $bin = "$OutName.bin"
$ys = @"
read_verilog uart_tx.v phase_lattice_uart.v phase_lattice_2d_glider_serial.v
chparam -set M $M phase_lattice_2d_glider_serial_top
chparam -set GENS $GENS phase_lattice_2d_glider_serial_top
chparam -set VACUUM $VACUUM phase_lattice_2d_glider_serial_top
chparam -set DEFECT $DEFECT phase_lattice_2d_glider_serial_top
chparam -set SEED_R $SEED_R phase_lattice_2d_glider_serial_top
chparam -set SEED_C $SEED_C phase_lattice_2d_glider_serial_top
synth_ice40 -top phase_lattice_2d_glider_serial_top -json $json
"@
& $YOSYS -l (Join-Path $ev "yosys.log") -p $ys
if ($LASTEXITCODE -ne 0) { throw "yosys failed" }
$pnrLog = Join-Path $ev "nextpnr.log"
cmd /c "`"$NEXTPNR`" --up5k --package sg48 --json `"$json`" --pcf icebreaker.pcf --asc `"$asc`" --report `"$ev\nextpnr_report.json`" > `"$pnrLog`" 2>&1"
if ($LASTEXITCODE -ne 0) { Get-Content $pnrLog -Tail 50; throw "nextpnr failed" }
Get-Content $pnrLog | Select-String "Max frequency|ICESTORM_LC" | ForEach-Object { $_.Line }
& $ICEPACK $asc $bin
$hash = (Get-FileHash -Algorithm SHA256 $bin).Hash.ToLower()
Copy-Item $bin (Join-Path $ev "icebreaker.bin") -Force
Set-Content (Join-Path $ev "icebreaker.bin.sha256") "$hash  icebreaker.bin" -NoNewline
& $ICEPROG $bin
$CaptureSeconds = [Math]::Max(60, [int](20 + $GENS / 20))
Start-Sleep -Seconds 2
$port = New-Object System.IO.Ports.SerialPort 'COM24',115200,'None',8,'One'
$port.Open()
$deadline = [DateTime]::UtcNow.AddSeconds($CaptureSeconds)
$sb = New-Object System.Text.StringBuilder
$banner = "PHASE LATTICE 2D GLIDER"
while ([DateTime]::UtcNow -lt $deadline) {
    if ($port.BytesToRead -gt 0) { [void]$sb.Append($port.ReadExisting()) }
    else { Start-Sleep -Milliseconds 20 }
    if ($sb.ToString() -match "(?s)$([regex]::Escape($banner)).*?LATTICE DONE") { break }
}
$port.Close()
$text = $sb.ToString()
Set-Content (Join-Path $ev "uart_capture_raw.txt") $text
$rx = [regex]::Match($text, "(?s)$([regex]::Escape($banner)).*?LATTICE DONE[^\r\n]*")
if (-not $rx.Success) { Write-Host $text.Substring([Math]::Max(0,$text.Length-400)); throw "UART incomplete" }
Set-Content (Join-Path $ev "uart_capture.txt") $rx.Value
Write-Host $rx.Value.Substring(0, [Math]::Min(400, $rx.Value.Length))
python verify_lattice_2d_glider.py (Join-Path $ev "uart_capture_raw.txt") $M $GENS $VACUUM $DEFECT $SEED_R $SEED_C
if ($LASTEXITCODE -ne 0) { throw "host mismatch" }
if ($rx.Value -notmatch "GLIDER PASS") { throw "GLIDER FAIL" }
@(
    "PHYSICALLY_VERIFIED=YES"
    "GATE=$OutName"
    "BOARD=iCEBreaker V1.1a"
    "UART=COM24 115200 8N1"
    "CLOCK_HZ=12000000"
    "BITSTREAM_SHA256=$hash"
    "M=$M"
    "GENS=$GENS"
    "ARCH=cell_serial"
    "MISMATCH_COUNT=0"
    "RESULT=serial glider MATCH host; F=01"
) | Set-Content (Join-Path $ev "PHYSICALLY_VERIFIED.txt")
Write-Host "RESULT: PHYSICALLY_VERIFIED $OutName"
