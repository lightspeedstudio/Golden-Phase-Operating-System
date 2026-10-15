# Flash + capture parameterized glider; host-verify every generation.
param(
    [ValidateSet(4, 8)]
    [int]$M = 4,
    [int]$GENS = 8,
    [int]$VACUUM = 1,
    [int]$DEFECT = 2,
    [int]$SEED_R = -1,
    [int]$SEED_C = -1,
    [int]$CaptureSeconds = 0,
    [string]$OutName = "",
    [switch]$SkipBuild
)
$ErrorActionPreference = "Stop"
$CAD = "C:\Tools\oss-cad-suite\oss-cad-suite"
$ICEPROG = Join-Path $CAD "bin\iceprog.exe"
$env:PATH = "$CAD\bin;$CAD\lib;$env:PATH"
Set-Location $PSScriptRoot

if ($SEED_R -lt 0) { $SEED_R = [int]($M / 2) }
if ($SEED_C -lt 0) { $SEED_C = [int]($M / 2) }
if ([string]::IsNullOrEmpty($OutName)) {
    $OutName = "phase_lattice_2d_glider_M${M}_G${GENS}_V${VACUUM}D${DEFECT}_S${SEED_R}${SEED_C}"
}
if ($CaptureSeconds -le 0) {
    # ~30 bytes/gen @ 115200 ≈ 350 gens/s; allow a missed first burst + 4s resend.
    $CaptureSeconds = [Math]::Max(45, [int](30 + ($GENS / 40) * 2))
}

$ev = Join-Path $PSScriptRoot "evidence\$OutName"
$bin = "$OutName.bin"
New-Item -ItemType Directory -Force -Path $ev | Out-Null

if (-not $SkipBuild) {
    & .\build-phase-lattice-2d-glider.ps1 -M $M -GENS $GENS -VACUUM $VACUUM -DEFECT $DEFECT -SEED_R $SEED_R -SEED_C $SEED_C -OutName $OutName
}

if (-not (Test-Path $bin)) { throw "missing $bin" }

$ports = [System.IO.Ports.SerialPort]::GetPortNames()
if ($ports -notcontains 'COM24') { throw "COM24 not present (ports: $($ports -join ', '))" }

$t0 = Get-Date
Write-Host "== iceprog $bin =="
& $ICEPROG $bin
if ($LASTEXITCODE -ne 0) { throw "iceprog failed" }

$hash = (Get-FileHash -Algorithm SHA256 $bin).Hash.ToLower()
Copy-Item $bin (Join-Path $ev "icebreaker.bin") -Force
Set-Content (Join-Path $ev "icebreaker.bin.sha256") "$hash  icebreaker.bin" -NoNewline

Start-Sleep -Seconds 2
$port = New-Object System.IO.Ports.SerialPort 'COM24',115200,'None',8,'One'
$port.Open()
$deadline = [DateTime]::UtcNow.AddSeconds($CaptureSeconds)
$sb = New-Object System.Text.StringBuilder
$banner = "PHASE LATTICE 2D GLIDER"
while ([DateTime]::UtcNow -lt $deadline) {
    if ($port.BytesToRead -gt 0) { [void]$sb.Append($port.ReadExisting()) }
    else { Start-Sleep -Milliseconds 20 }
    # Require banner+DONE so a mid-stream open cannot stop on a partial soak.
    if ($sb.ToString() -match "(?s)$([regex]::Escape($banner)).*?LATTICE DONE") { break }
}
$port.Close()
$text = $sb.ToString()
$elapsed = ((Get-Date) - $t0).TotalSeconds
Set-Content (Join-Path $ev "uart_capture_raw.txt") $text

$rx = [regex]::Match($text, "(?s)$([regex]::Escape($banner)).*?LATTICE DONE[^\r\n]*")
if (-not $rx.Success) { Write-Host $text.Substring([Math]::Max(0, $text.Length - 500)); throw "UART evidence incomplete" }
Set-Content (Join-Path $ev "uart_capture.txt") $rx.Value
Write-Host ($rx.Value.Substring(0, [Math]::Min(500, $rx.Value.Length)))
if ($rx.Value.Length -gt 500) { Write-Host "... ($($rx.Value.Length) chars total)" }
$py = Get-Command python -ErrorAction SilentlyContinue
if (-not $py) { $py = Get-Command python3 -ErrorAction SilentlyContinue }
& $py.Source verify_lattice_2d_glider.py (Join-Path $ev "uart_capture_raw.txt") $M $GENS $VACUUM $DEFECT $SEED_R $SEED_C
if ($LASTEXITCODE -ne 0) { throw "host golden mismatch" }
if ($rx.Value -notmatch "GLIDER PASS") { throw "FPGA reported GLIDER FAIL" }

$cells = $M * $M * $GENS
@(
    "PHYSICALLY_VERIFIED=YES"
    "GATE=$OutName"
    "BOARD=iCEBreaker V1.1a"
    "UART=COM24 115200 8N1"
    "CLOCK_HZ=12000000"
    "BITSTREAM_SHA256=$hash"
    "M=$M"
    "GENS=$GENS"
    "VACUUM=$VACUUM"
    "DEFECT=$DEFECT"
    "SEED_R=$SEED_R"
    "SEED_C=$SEED_C"
    "CELL_UPDATES=$cells"
    "TEST_DURATION_S=$([Math]::Round($elapsed,2))"
    "MISMATCH_COUNT=0"
    "RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS"
) | Set-Content (Join-Path $ev "PHYSICALLY_VERIFIED.txt")

Write-Host "RESULT: PHYSICALLY_VERIFIED $OutName duration=${elapsed}s cells=$cells"
