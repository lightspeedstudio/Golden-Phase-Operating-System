param(
    [ValidateSet(0,1,2,3,4)]
    [int]$FaultMode = 1,
    [int]$GENS = 8
)
$ErrorActionPreference = "Stop"
$CAD = "C:\Tools\oss-cad-suite\oss-cad-suite"
$YOSYS = Join-Path $CAD "bin\yosys.exe"
$NEXTPNR = Join-Path $CAD "bin\nextpnr-ice40.exe"
$ICEPACK = Join-Path $CAD "bin\icepack.exe"
$ICEPROG = Join-Path $CAD "bin\iceprog.exe"
$env:PATH = "$CAD\bin;$CAD\lib;$env:PATH"
Set-Location $PSScriptRoot
$OutName = "phase_lattice_2d_fault_M${FaultMode}_G${GENS}"
$ev = Join-Path $PSScriptRoot "evidence\$OutName"
New-Item -ItemType Directory -Force -Path $ev | Out-Null
$json="$OutName.json"; $asc="$OutName.asc"; $bin="$OutName.bin"
$ys = @"
read_verilog uart_tx.v phase_lattice_uart.v phase_lattice_2d_fault.v
chparam -set FAULT_MODE $FaultMode phase_lattice_2d_fault_top
chparam -set GENS $GENS phase_lattice_2d_fault_top
synth_ice40 -top phase_lattice_2d_fault_top -json $json
"@
& $YOSYS -l (Join-Path $ev "yosys.log") -p $ys
if ($LASTEXITCODE -ne 0) { throw "yosys failed" }
$pnrLog = Join-Path $ev "nextpnr.log"
cmd /c "`"$NEXTPNR`" --up5k --package sg48 --json `"$json`" --pcf icebreaker.pcf --asc `"$asc`" --report `"$ev\nextpnr_report.json`" > `"$pnrLog`" 2>&1"
if ($LASTEXITCODE -ne 0) { Get-Content $pnrLog -Tail 40; throw "nextpnr failed" }
& $ICEPACK $asc $bin
$hash = (Get-FileHash -Algorithm SHA256 $bin).Hash.ToLower()
Copy-Item $bin (Join-Path $ev "icebreaker.bin") -Force
Set-Content (Join-Path $ev "icebreaker.bin.sha256") "$hash  icebreaker.bin" -NoNewline
& $ICEPROG $bin
Start-Sleep -Seconds 3
$port = New-Object System.IO.Ports.SerialPort 'COM24',115200,'None',8,'One'
$port.ReadTimeout = 200
$port.Open(); $sb = New-Object System.Text.StringBuilder
$deadline = [DateTime]::UtcNow.AddSeconds(90)
while ([DateTime]::UtcNow -lt $deadline) {
    if ($port.BytesToRead -gt 0) { [void]$sb.Append($port.ReadExisting()) }
    else { Start-Sleep -Milliseconds 20 }
    if ($sb.ToString() -match "FAULT DONE") { break }
}
$port.Close()
$text = $sb.ToString()
Set-Content (Join-Path $ev "uart_capture_raw.txt") $text
Set-Content (Join-Path $ev "uart_capture.txt") $text
Write-Host ("UART_BYTES={0}" -f $text.Length)
if ($text.Length -gt 0) { Write-Host $text.Substring(0, [Math]::Min(500, $text.Length)) }
else { Write-Host "UART empty - check COM24 / board power" }
python verify_lattice_2d_fault.py (Join-Path $ev "uart_capture_raw.txt") $FaultMode
if ($LASTEXITCODE -ne 0) { throw "fault verify failed" }
@(
    "PHYSICALLY_VERIFIED=YES"
    "GATE=$OutName"
    "BOARD=iCEBreaker V1.1a"
    "UART=COM24 115200 8N1"
    "BITSTREAM_SHA256=$hash"
    "FAULT_MODE=$FaultMode"
    "RESULT=fault inject UART MATCH host detect check"
) | Set-Content (Join-Path $ev "PHYSICALLY_VERIFIED.txt")
Write-Host "RESULT: PHYSICALLY_VERIFIED $OutName"
