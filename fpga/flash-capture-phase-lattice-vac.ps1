# Flash + capture vacuum/lock lattice (COM24); require VACUUM PASS
param(
    [ValidateSet(0, 3)]
    [int]$Phi = 0
)
$ErrorActionPreference = "Stop"
$CAD = "C:\Tools\oss-cad-suite\oss-cad-suite"
$env:PATH = "$CAD\bin;$CAD\lib;$env:PATH"
Set-Location $PSScriptRoot

$tag = if ($Phi -eq 0) { "phi0" } else { "phi3" }
$bin = "phase_lattice_$tag.bin"
$ev = Join-Path $PSScriptRoot "evidence\phase_lattice_$tag"
New-Item -ItemType Directory -Force -Path $ev | Out-Null

if (-not (Test-Path $bin)) {
    throw "missing $bin - run build-phase-lattice-vac.ps1 -Phi $Phi first"
}

$ports = [System.IO.Ports.SerialPort]::GetPortNames()
if ($ports -notcontains 'COM24') {
    throw "COM24 not present (ports: $($ports -join ', '))"
}

Write-Host "== iceprog $bin =="
& iceprog $bin
if ($LASTEXITCODE -ne 0) { throw "iceprog failed" }

Copy-Item $bin (Join-Path $ev "icebreaker.bin") -Force
$hash = (Get-FileHash -Algorithm SHA256 $bin).Hash.ToLower()
Set-Content (Join-Path $ev "icebreaker.bin.sha256") "$hash  icebreaker.bin" -NoNewline

Start-Sleep -Seconds 2
$port = New-Object System.IO.Ports.SerialPort 'COM24',115200,'None',8,'One'
$port.Open()
$deadline = [DateTime]::UtcNow.AddSeconds(25)
$sb = New-Object System.Text.StringBuilder
while ([DateTime]::UtcNow -lt $deadline) {
    if ($port.BytesToRead -gt 0) { [void]$sb.Append($port.ReadExisting()) }
    else { Start-Sleep -Milliseconds 30 }
}
$port.Close()
$text = $sb.ToString()
Set-Content (Join-Path $ev "uart_capture_raw.txt") $text

$banner = "PHASE VACUUM Phi$Phi"
$m = [regex]::Match($text, "(?s)$banner.*?LATTICE DONE[^\r\n]*")
if (-not $m.Success) { Write-Host $text; throw "UART evidence incomplete for Phi$Phi" }
Set-Content (Join-Path $ev "uart_capture.txt") $m.Value
Write-Host $m.Value

if ($text -notmatch "VACUUM PASS") { throw "VACUUM PASS marker missing" }
# All generation lines must show D=00
$bad = [regex]::Matches($text, 'G\d\d S=([0-3]{16}) D=(\d\d)') | Where-Object {
    $_.Groups[2].Value -ne '00' -or ($_.Groups[1].Value.ToCharArray() | Where-Object { $_ -ne [char]"$Phi" }).Count -gt 0
}
if ($bad.Count -gt 0) { throw "vacuum field/defect check failed ($($bad.Count) bad lines)" }

@(
    "PHYSICALLY_VERIFIED=YES"
    "GATE=phase_lattice_vacuum_phi$Phi"
    "BOARD=iCEBreaker V1.1a"
    "UART=COM24 115200 8N1"
    "BITSTREAM_SHA256=$hash"
    "RESULT=VACUUM PASS; homogeneous Phi$Phi fixed + D=00 for all gens"
) | Set-Content (Join-Path $ev "PHYSICALLY_VERIFIED.txt")

Write-Host "RESULT: PHYSICALLY_VERIFIED Phi$Phi"
