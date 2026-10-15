# Flash + capture scaled 1-D phase lattice (COM24); verify vs Python reference
param(
    [ValidateSet(32, 64, 128)]
    [int]$N = 32
)
$ErrorActionPreference = "Stop"
$CAD = "C:\Tools\oss-cad-suite\oss-cad-suite"
$env:PATH = "$CAD\bin;$CAD\lib;$env:PATH"
Set-Location $PSScriptRoot

$asc = Join-Path $PSScriptRoot "evidence\phase_lattice_scale\N$N\out.asc"
$bin = "phase_lattice_N$N.bin"
$ev = Join-Path $PSScriptRoot "evidence\phase_lattice_scale\N${N}_physical"
New-Item -ItemType Directory -Force -Path $ev | Out-Null

if (-not (Test-Path $asc)) {
    throw "missing $asc - run build-phase-lattice-scale.ps1 first"
}

Write-Host "== icepack N=$N =="
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

# N=128 lines are longer; allow more capture time
$secs = if ($N -ge 128) { 90 } elseif ($N -ge 64) { 45 } else { 30 }
Start-Sleep -Seconds 2
$port = New-Object System.IO.Ports.SerialPort 'COM24',115200,'None',8,'One'
$port.Open()
$deadline = [DateTime]::UtcNow.AddSeconds($secs)
$sb = New-Object System.Text.StringBuilder
while ([DateTime]::UtcNow -lt $deadline) {
    if ($port.BytesToRead -gt 0) { [void]$sb.Append($port.ReadExisting()) }
    else { Start-Sleep -Milliseconds 30 }
}
$port.Close()
$text = $sb.ToString()
Set-Content (Join-Path $ev "uart_capture_raw.txt") $text

$banner = ("PHASE LATTICE N={0:D3}" -f $N)
$m = [regex]::Match($text, "(?s)$([regex]::Escape($banner)).*?LATTICE DONE[^\r\n]*")
if (-not $m.Success) { Write-Host $text; throw "UART evidence incomplete for N=$N" }
Set-Content (Join-Path $ev "uart_capture.txt") $m.Value
Write-Host $m.Value

$py = Get-Command python -ErrorAction SilentlyContinue
if ($null -eq $py) { $py = Get-Command python3 -ErrorAction SilentlyContinue }
if ($null -eq $py) { throw "python not found" }
& $py.Source verify_lattice_scale.py $N (Join-Path $ev "uart_capture_raw.txt") 8 4
if ($LASTEXITCODE -ne 0) { throw "verify_lattice_scale.py mismatch for N=$N" }

if ($text -notmatch "CLOCK PASS") { throw "CLOCK PASS missing for N=$N" }

@(
    "PHYSICALLY_VERIFIED=YES"
    "GATE=phase_lattice_scale_n$N"
    "BOARD=iCEBreaker V1.1a"
    "UART=COM24 115200 8N1"
    "BITSTREAM_SHA256=$hash"
    "RESULT=N=$N physical UART matches verify_lattice_scale.py; CLOCK PASS"
) | Set-Content (Join-Path $ev "PHYSICALLY_VERIFIED.txt")

Write-Host "RESULT: PHYSICALLY_VERIFIED N=$N"
