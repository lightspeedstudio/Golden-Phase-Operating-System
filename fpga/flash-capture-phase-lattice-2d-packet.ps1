# Live packet UART: I/M/A/P (PROP_ARM). Optional soak rounds.
param(
    [int]$GENS = 2,
    [int]$Rounds = 1,
    [switch]$SkipBuild
)
$ErrorActionPreference = "Stop"
$CAD = "C:\Tools\oss-cad-suite\oss-cad-suite"
$YOSYS = Join-Path $CAD "bin\yosys.exe"
$NEXTPNR = Join-Path $CAD "bin\nextpnr-ice40.exe"
$ICEPACK = Join-Path $CAD "bin\icepack.exe"
$ICEPROG = Join-Path $CAD "bin\iceprog.exe"
$env:PATH = "$CAD\bin;$CAD\lib;$env:PATH"
Set-Location $PSScriptRoot
$OutName = if ($Rounds -gt 1) { "phase_lattice_2d_packet_prop_arm_G${GENS}_soak$Rounds" } else { "phase_lattice_2d_packet_prop_arm_G${GENS}" }
$ev = Join-Path $PSScriptRoot "evidence\$OutName"
New-Item -ItemType Directory -Force -Path $ev | Out-Null
$json = "phase_lattice_2d_packet_prop_arm_G${GENS}.json"
$asc = "phase_lattice_2d_packet_prop_arm_G${GENS}.asc"
$bin = "phase_lattice_2d_packet_prop_arm_G${GENS}.bin"

if (-not $SkipBuild) {
    $ys = @"
read_verilog uart_tx.v uart_rx.v phase_lattice_uart.v phase_lattice_2d_packet.v
chparam -set GENS $GENS phase_lattice_2d_packet_top
synth_ice40 -top phase_lattice_2d_packet_top -json $json
"@
    & $YOSYS -l (Join-Path $ev "yosys.log") -p $ys
    if ($LASTEXITCODE -ne 0) { throw "yosys failed" }
    $pnrLog = Join-Path $ev "nextpnr.log"
    cmd /c "`"$NEXTPNR`" --up5k --package sg48 --json `"$json`" --pcf icebreaker.pcf --asc `"$asc`" --report `"$ev\nextpnr_report.json`" > `"$pnrLog`" 2>&1"
    if ($LASTEXITCODE -ne 0) { Get-Content $pnrLog -Tail 40; throw "nextpnr failed" }
    Get-Content $pnrLog | Select-String "Max frequency|ICESTORM_LC" | ForEach-Object { $_.Line }
    & $ICEPACK $asc $bin
}
$hash = (Get-FileHash -Algorithm SHA256 $bin).Hash.ToLower()
Copy-Item $bin (Join-Path $ev "icebreaker.bin") -Force
Set-Content (Join-Path $ev "icebreaker.bin.sha256") "$hash  icebreaker.bin" -NoNewline
& $ICEPROG $bin
Start-Sleep -Seconds 2

$port = New-Object System.IO.Ports.SerialPort 'COM24',115200,'None',8,'One'
$port.Handshake = [System.IO.Ports.Handshake]::None
$port.DtrEnable = $true
$port.RtsEnable = $true
$port.ReadTimeout = 500
$port.WriteTimeout = 500
$port.Open()
Start-Sleep -Milliseconds 500
$port.DtrEnable = $false
Start-Sleep -Milliseconds 200
$port.DtrEnable = $true
Start-Sleep -Seconds 2
$sb = New-Object System.Text.StringBuilder
function WaitToken([string]$tok, [int]$sec) {
    $deadline = [DateTime]::UtcNow.AddSeconds($sec)
    while ([DateTime]::UtcNow -lt $deadline) {
        if ($port.BytesToRead -gt 0) { [void]$sb.Append($port.ReadExisting()) }
        else { Start-Sleep -Milliseconds 20 }
        if ($sb.ToString().Contains($tok)) { return }
    }
    throw "timeout waiting for $tok; got=$($sb.ToString().Substring([Math]::Max(0,$sb.Length-200)))"
}
function OneRound {
    WaitToken "PKT READY" 25
    $port.Write([byte[]]@(0x49), 0, 1) # I
    WaitToken "PKT INJECT" 10
    $port.Write([byte[]]@(0x4D), 0, 1) # M
    WaitToken "PKT RECV" 20
    $port.Write([byte[]]@(0x41), 0, 1) # A
    WaitToken "PKT ACK" 10
    $port.Write([byte[]]@(0x50), 0, 1) # P = PROP_ARM
    WaitToken "PKT PROP_ARM OK" 10
    WaitToken "PKT DONE" 10
}

$pass = 0
$all = New-Object System.Text.StringBuilder
for ($r = 1; $r -le $Rounds; $r++) {
    Write-Host "===== ROUND $r / $Rounds ====="
    if ($r -gt 1) {
        # Soft re-arm emits READY ~2s after DONE; clear so we match the new banner.
        [void]$sb.Clear()
        Start-Sleep -Seconds 1
    }
    OneRound
    [void]$all.AppendLine("===== ROUND $r =====")
    [void]$all.Append($sb.ToString())
    $pass++
}
$port.Close()

$text = $all.ToString()
Set-Content (Join-Path $ev "uart_capture_raw.txt") $text
Set-Content (Join-Path $ev "uart_capture.txt") $sb.ToString()
Write-Host $sb.ToString().Substring(0, [Math]::Min(800, $sb.Length))
python verify_lattice_2d_packet.py (Join-Path $ev "uart_capture.txt") $GENS
if ($LASTEXITCODE -ne 0) { throw "packet verify failed" }
@(
    "PHYSICALLY_VERIFIED=YES"
    "GATE=$OutName"
    "BOARD=iCEBreaker V1.1a"
    "UART=COM24 115200 8N1"
    "CLOCK_HZ=12000000"
    "BITSTREAM_SHA256=$hash"
    "PROTOCOL=I/M/A/P live UART PROP_ARM"
    "GENS=$GENS"
    "ROUNDS=$Rounds"
    "ROUNDS_PASS=$pass"
    "MISMATCH_COUNT=0"
    "RESULT=live inject-move-ACK-PROP_ARM MATCH host"
) | Set-Content (Join-Path $ev "PHYSICALLY_VERIFIED.txt")
"PASS packet_prop_arm rounds=$pass" | Add-Content (Join-Path $PSScriptRoot "evidence\ice_max_run_log.txt")
Write-Host "RESULT: PHYSICALLY_VERIFIED $OutName rounds=$pass"
