# Build + flash GPL algebra 64-triple golden (Phase 2).
$ErrorActionPreference = "Stop"
$CAD = "C:\Tools\oss-cad-suite\oss-cad-suite"
$env:PATH = "$CAD\bin;$CAD\lib;$env:PATH"
Set-Location $PSScriptRoot

$ev = Join-Path $PSScriptRoot "evidence\gpl_algebra_64"
New-Item -ItemType Directory -Force -Path $ev | Out-Null
$json = "gpl_algebra_64.json"
$asc = "gpl_algebra_64.asc"
$bin = "gpl_algebra_64.bin"
$YOSYS = Join-Path $CAD "bin\yosys.exe"
$NEXTPNR = Join-Path $CAD "bin\nextpnr-ice40.exe"
$ICEPACK = Join-Path $CAD "bin\icepack.exe"
$ICEPROG = Join-Path $CAD "bin\iceprog.exe"

Write-Host "== yosys gpl_algebra_64 =="
& $YOSYS -l (Join-Path $ev "yosys.log") -p "read_verilog uart_tx.v phase_lattice_uart.v gpl_algebra_64.v; synth_ice40 -top gpl_algebra_64_top -json $json"
if ($LASTEXITCODE -ne 0) { throw "yosys failed" }

$pnrLog = Join-Path $ev "nextpnr.log"
cmd /c "`"$NEXTPNR`" --up5k --package sg48 --json `"$json`" --pcf icebreaker.pcf --asc `"$asc`" --report `"$(Join-Path $ev 'nextpnr_report.json')`" > `"$pnrLog`" 2>&1"
if ($LASTEXITCODE -ne 0) { Get-Content $pnrLog -Tail 40; throw "nextpnr failed" }
& $ICEPACK $asc $bin

$hash = (Get-FileHash -Algorithm SHA256 $bin).Hash.ToLower()
Copy-Item $bin (Join-Path $ev "icebreaker.bin") -Force
Set-Content (Join-Path $ev "icebreaker.bin.sha256") "$hash  icebreaker.bin" -NoNewline

if ([System.IO.Ports.SerialPort]::GetPortNames() -notcontains 'COM24') {
    throw "COM24 missing"
}

$t0 = Get-Date
& $ICEPROG $bin
Start-Sleep -Seconds 2
$port = New-Object System.IO.Ports.SerialPort 'COM24',115200,'None',8,'One'
$port.Open()
$deadline = [DateTime]::UtcNow.AddSeconds(40)
$sb = New-Object System.Text.StringBuilder
while ([DateTime]::UtcNow -lt $deadline) {
    if ($port.BytesToRead -gt 0) { [void]$sb.Append($port.ReadExisting()) }
    else { Start-Sleep -Milliseconds 20 }
    if ($sb.ToString() -match "ALGEBRA DONE") { break }
}
$port.Close()
$text = $sb.ToString()
$elapsed = ((Get-Date) - $t0).TotalSeconds
Set-Content (Join-Path $ev "uart_capture_raw.txt") $text
$m = [regex]::Match($text, "(?s)GPL ALGEBRA 64 TRIPLES.*?ALGEBRA DONE[^\r\n]*")
if (-not $m.Success) { Write-Host $text; throw "UART incomplete" }
Set-Content (Join-Path $ev "uart_capture.txt") $m.Value
Write-Host $m.Value

python verify_gpl_algebra_64.py (Join-Path $ev "uart_capture_raw.txt")
if ($LASTEXITCODE -ne 0) { throw "algebra verify failed" }

@(
    "PHYSICALLY_VERIFIED=YES"
    "GATE=gpl_algebra_64"
    "BOARD=iCEBreaker V1.1a"
    "UART=COM24 115200 8N1"
    "CLOCK_HZ=12000000"
    "BITSTREAM_SHA256=$hash"
    "TEST_DURATION_S=$([Math]::Round($elapsed,2))"
    "MISMATCH_COUNT=0"
    "RESULT=64/64 triples ab,L,R,p MATCH host; ALGEBRA PASS"
) | Set-Content (Join-Path $ev "PHYSICALLY_VERIFIED.txt")

Write-Host "RESULT: PHYSICALLY_VERIFIED gpl_algebra_64"
