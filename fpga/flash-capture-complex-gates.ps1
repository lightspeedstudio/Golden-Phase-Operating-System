# Flash and capture complex gates suite UART (COM24)
$ErrorActionPreference = "Stop"
$CAD = "C:\Tools\oss-cad-suite\oss-cad-suite"
$env:PATH = "$CAD\bin;$CAD\lib;$env:PATH"
Set-Location $PSScriptRoot
$ev = Join-Path $PSScriptRoot "evidence\complex_gates"
New-Item -ItemType Directory -Force -Path $ev | Out-Null

if (-not (Test-Path icebreaker.bin)) {
    throw "missing icebreaker.bin - run build-icebreaker.ps1 first"
}

Write-Host "== iceprog =="
& iceprog icebreaker.bin
if ($LASTEXITCODE -ne 0) {
    throw "iceprog failed - unplug/replug USB and retry"
}

Copy-Item icebreaker.bin (Join-Path $ev "icebreaker.bin") -Force
$hash = (Get-FileHash -Algorithm SHA256 icebreaker.bin).Hash.ToLower()
Set-Content (Join-Path $ev "icebreaker.bin.sha256") "$hash  icebreaker.bin" -NoNewline

Start-Sleep -Seconds 2
$port = New-Object System.IO.Ports.SerialPort 'COM24',115200,'None',8,'One'
$port.Open()
$deadline = [DateTime]::UtcNow.AddSeconds(50)
$sb = New-Object System.Text.StringBuilder
while ([DateTime]::UtcNow -lt $deadline) {
    if ($port.BytesToRead -gt 0) { [void]$sb.Append($port.ReadExisting()) }
    else { Start-Sleep -Milliseconds 30 }
}
$port.Close()
$text = $sb.ToString()
Set-Content (Join-Path $ev "uart_capture_raw.txt") $text

$m = [regex]::Match($text, '(?s)GPOS/iCE COMPOSE TEST.*?GPL VM (PASS|FAIL)[^\r\n]*')
if (-not $m.Success) { Write-Host $text; throw "UART evidence incomplete" }
Set-Content (Join-Path $ev "uart_capture.txt") $m.Value
Write-Host $m.Value

$ok = ($text -match "COMPOSE 16/16 PASS") -and
      ($text -match "REFLECT 4/4 PASS") -and
      ($text -match "TOPOLOGY 64/64 PASS") -and
      ($text -match "T_SIGN 4/4 PASS") -and
      ($text -match "T_ENERGY 4/4 PASS") -and
      ($text -match "PARALLEL 4/4 PASS") -and
      ($text -match "GPL VM PASS")
if (-not $ok) { throw "marker check failed" }

@(
    "PHYSICALLY_VERIFIED=YES"
    "GATE=compose+reflect+topology+t_sign+t_energy+parallel4+gpl_vm_complex"
    "BOARD=iCEBreaker V1.1a"
    "UART=COM24 115200 8N1"
    "BITSTREAM_SHA256=$hash"
    "RESULT=COMPOSE 16/16; REFLECT 4/4; TOPOLOGY 64/64; T_SIGN 4/4; T_ENERGY 4/4; PARALLEL 4/4; GPL VM PASS"
) | Set-Content (Join-Path $ev "PHYSICALLY_VERIFIED.txt")

Write-Host "RESULT: PHYSICALLY_VERIFIED"
