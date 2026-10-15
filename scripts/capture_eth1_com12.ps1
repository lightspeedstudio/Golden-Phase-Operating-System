# ETH-1 COM12 capture: PHY ID + link DOWN (no cable) + link UP (cable) with speed/duplex.
#
# Requires a board already running a kairos_zynq image that prints:
#   gem link: ... speed=... duplex=...
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts\capture_eth1_com12.ps1
#
# Operator prompts pause for cable disconnect / connect. Does not flash BOOT.

param(
    [string]$Port = "COM12",
    [int]$Baud = 115200,
    [int]$CmdTimeoutSec = 15,
    [int]$BootTimeoutSec = 45,
    [string]$OutRoot = "$PSScriptRoot\..\fpga\zynq\evidence\kairos_zynq_boot"
)

$ErrorActionPreference = "Stop"
$prompt = "gpos> "
$runId = "eth1_" + (Get-Date -Format "yyyyMMdd_HHmmss")
$outDir = Join-Path $OutRoot $runId
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
$transcriptFile = Join-Path $outDir "transcript.txt"
$reportFile = Join-Path $outDir "ETH1_REPORT.md"

$sp = New-Object System.IO.Ports.SerialPort $Port, $Baud, ([System.IO.Ports.Parity]::None), 8, ([System.IO.Ports.StopBits]::One)
$sp.ReadTimeout = 300
$sp.NewLine = "`n"
$sp.DtrEnable = $true
$sp.RtsEnable = $true
$script:sb = New-Object System.Text.StringBuilder

function Read-Available {
    try {
        $chunk = $sp.ReadExisting()
        if ($chunk) {
            [void]$script:sb.Append($chunk)
            Write-Host -NoNewline $chunk
        }
    } catch {}
}

function Get-PromptCount {
    return ([regex]::Matches($script:sb.ToString(), [regex]::Escape($prompt))).Count
}

function Wait-ForPrompt {
    param([int]$Target, [int]$TimeoutSec)
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while ((Get-Date) -lt $deadline) {
        Read-Available
        if ((Get-PromptCount) -ge $Target) { return $true }
        Start-Sleep -Milliseconds 40
    }
    return $false
}

function Send-Cmd {
    param([string]$Cmd)
    $before = Get-PromptCount
    Write-Host "`n>>> $Cmd"
    $sp.Write("$Cmd`r")
    if (-not (Wait-ForPrompt -Target ($before + 1) -TimeoutSec $CmdTimeoutSec)) {
        Write-Host "`n[WARN] no prompt after '$Cmd'"
        return $false
    }
    return $true
}

function Wait-Operator {
    param([string]$Message)
    Write-Host ""
    Write-Host "============================================================" -ForegroundColor Yellow
    Write-Host $Message -ForegroundColor Yellow
    Write-Host "Press Enter to continue..." -ForegroundColor Yellow
    Write-Host "============================================================" -ForegroundColor Yellow
    [void][Console]::ReadLine()
}

try {
    $sp.Open()
    Write-Host "Opened $Port @ $Baud"
    Wait-Operator "Ensure kairos_zynq is at gpos> (press board RESET if needed), then Enter."

    if (-not (Wait-ForPrompt -Target 1 -TimeoutSec $BootTimeoutSec)) {
        # Try waking the prompt with Enter.
        $sp.Write("`r")
        if (-not (Wait-ForPrompt -Target 1 -TimeoutSec 10)) {
            throw "No gpos> prompt on $Port — flash/boot kairos_zynq first."
        }
    }

    [void](Send-Cmd "net probe")
    [void](Send-Cmd "net phy")

    Wait-Operator "ETH-1 DOWN: DISCONNECT the Ethernet cable from the PZ7020, wait ~3s, then Enter."
    [void](Send-Cmd "net link")

    Wait-Operator "ETH-1 UP: CONNECT the Ethernet cable to a live link partner, wait for link LEDs (~5s), then Enter."
    [void](Send-Cmd "net link")

    Start-Sleep -Milliseconds 500
    Read-Available
}
finally {
    if ($sp.IsOpen) { $sp.Close() }
    $sp.Dispose()
}

$text = $script:sb.ToString()
$text | Set-Content $transcriptFile -Encoding UTF8

$phyOk = $text -match 'gem phy: addr=\d+ id1=0x001[Cc] id2=0x[Cc]916'
$downOk = $text -match 'gem link: phy=\d+ DOWN'
$upOk = $text -match 'gem link: phy=\d+ UP'
$speedOk = $text -match 'gem link: phy=\d+ UP[^\r\n]*speed=(10|100|1000)'
$duplexOk = $text -match 'gem link: phy=\d+ UP[^\r\n]*duplex=(full|half)'

$report = @"
# ETH-1 COM12 evidence — $runId

## Checks

| Check | Result |
|-------|--------|
| PHY ID RTL8211F (0x001C/0xC916) | $(if ($phyOk) { 'PASS' } else { 'FAIL' }) |
| Cable disconnected → link DOWN | $(if ($downOk) { 'PASS' } else { 'FAIL' }) |
| Cable connected → link UP | $(if ($upOk) { 'PASS' } else { 'FAIL' }) |
| Negotiated speed reported | $(if ($speedOk) { 'PASS' } else { 'FAIL' }) |
| Negotiated duplex reported | $(if ($duplexOk) { 'PASS' } else { 'FAIL' }) |

## Notes

- External TX/RX not exercised (ETH-1 only).
- Transcript: ``transcript.txt``

## Verdict

$(if ($phyOk -and $downOk -and $upOk -and $speedOk -and $duplexOk) { 'ETH-1 PASS' } else { 'ETH-1 INCOMPLETE — see FAIL rows; do not claim physical PASS' })
"@
$report | Set-Content $reportFile -Encoding UTF8
Write-Host "`nSaved: $transcriptFile"
Write-Host "Report: $reportFile"
if (-not ($phyOk -and $downOk -and $upOk -and $speedOk -and $duplexOk)) {
    exit 1
}
exit 0
