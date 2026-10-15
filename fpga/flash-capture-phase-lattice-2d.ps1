# Flash + capture 2-D torus lattice smoke (COM24). Seals fabric-alive, not glider claim.
param(
    [ValidateSet(4, 8)]
    [int]$M = 4
)
$ErrorActionPreference = "Stop"
$CAD = "C:\Tools\oss-cad-suite\oss-cad-suite"
$env:PATH = "$CAD\bin;$CAD\lib;$env:PATH"
Set-Location $PSScriptRoot

$asc = Join-Path $PSScriptRoot "evidence\phase_lattice_scale\2d_M$M\out.asc"
$bin = "phase_lattice_2d_M$M.bin"
$ev = Join-Path $PSScriptRoot "evidence\phase_lattice_2d_M$M"
New-Item -ItemType Directory -Force -Path $ev | Out-Null

if (-not (Test-Path $asc)) {
    throw "missing $asc - run build-phase-lattice-scale.ps1 first"
}

Write-Host "== icepack 2D M=$M =="
& icepack $asc $bin
if ($LASTEXITCODE -ne 0) { throw "icepack failed" }

$ports = [System.IO.Ports.SerialPort]::GetPortNames()
if ($ports -notcontains 'COM24') {
    throw "COM24 not present (ports: $($ports -join ', '))"
}

Write-Host "== iceprog $bin =="
& iceprog $bin
if ($LASTEXITCODE -ne 0) { throw "iceprog failed" }

Copy-Item $bin (Join-Path $ev "icebreaker.bin") -Force
Copy-Item $asc (Join-Path $ev "icebreaker.asc") -Force
$hash = (Get-FileHash -Algorithm SHA256 $bin).Hash.ToLower()
Set-Content (Join-Path $ev "icebreaker.bin.sha256") "$hash  icebreaker.bin" -NoNewline

Start-Sleep -Seconds 2
$port = New-Object System.IO.Ports.SerialPort 'COM24',115200,'None',8,'One'
$port.Open()
$deadline = [DateTime]::UtcNow.AddSeconds(35)
$sb = New-Object System.Text.StringBuilder
while ([DateTime]::UtcNow -lt $deadline) {
    if ($port.BytesToRead -gt 0) { [void]$sb.Append($port.ReadExisting()) }
    else { Start-Sleep -Milliseconds 30 }
}
$port.Close()
$text = $sb.ToString()
Set-Content (Join-Path $ev "uart_capture_raw.txt") $text

$gridLabel = "{0:D2}x{0:D2}" -f $M
$banner = "PHASE LATTICE 2D $gridLabel"
$escaped = [regex]::Escape($banner)
$rx = [regex]::Match($text, "(?s)$escaped.*?LATTICE DONE[^\r\n]*")
if (-not $rx.Success) { Write-Host $text; throw "UART evidence incomplete for 2D M=$M" }
Set-Content (Join-Path $ev "uart_capture.txt") $rx.Value
Write-Host $rx.Value

$gens = [regex]::Matches($rx.Value, 'G\d\d S=[0-3]+ D=\d{3}')
if ($gens.Count -lt 1) { throw "no generation lines in 2D capture" }

@(
    "PHYSICALLY_VERIFIED=YES"
    "GATE=phase_lattice_2d_M$M"
    "BOARD=iCEBreaker V1.1a"
    "UART=COM24 115200 8N1"
    "BITSTREAM_SHA256=$hash"
    "RESULT=2-D ${gridLabel} fabric alive on silicon ($($gens.Count) gen lines); not a glider claim"
) | Set-Content (Join-Path $ev "PHYSICALLY_VERIFIED.txt")

Write-Host "RESULT: PHYSICALLY_VERIFIED 2D M=$M ($($gens.Count) gens)"
