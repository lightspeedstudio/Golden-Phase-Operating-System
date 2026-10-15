# PN-WIRE-1 COM12 capture: net wire init/ping/wrap (+ optional recover).
# Start scripts/pn_wire1_host_peer.py on the host NIC BEFORE running this.
#
#   powershell -ExecutionPolicy Bypass -File scripts\capture_pn_wire1_com12.ps1

param(
    [string]$Port = "COM12",
    [int]$Baud = 115200,
    [int]$CmdTimeoutSec = 20,
    [int]$BootTimeoutSec = 45,
    [string]$OutRoot = "$PSScriptRoot\..\fpga\zynq\evidence\kairos_zynq_boot"
)

$ErrorActionPreference = "Stop"
$prompt = "gpos> "
$runId = "pn_wire1_" + (Get-Date -Format "yyyyMMdd_HHmmss")
$outDir = Join-Path $OutRoot $runId
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
$transcriptFile = Join-Path $outDir "transcript.txt"
$reportFile = Join-Path $outDir "PN_WIRE1_REPORT.md"

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

Write-Host "============================================================"
Write-Host "PN-WIRE-1 capture on $Port @ $Baud"
Write-Host "Start host peer FIRST, then press RESET if needed."
Write-Host "Output: $outDir"
Write-Host "============================================================"

$sp.Open()
try {
    Write-Host "Waiting for $prompt (${BootTimeoutSec}s)..."
    if (-not (Wait-ForPrompt -Target 1 -TimeoutSec $BootTimeoutSec)) {
        $sp.Write("`r")
        if (-not (Wait-ForPrompt -Target 1 -TimeoutSec 15)) {
            throw "No gpos> prompt — is PN-WIRE-1 BOOT.BIN running?"
        }
    }

    foreach ($cmd in @(
            "net link",
            "net wire init",
            "net wire status",
            "net wire ping debug",
            "net wire ping",
            "net wire wrap",
            "net wire status"
        )) {
        [void](Send-Cmd $cmd)
    }

    Write-Host ""
    Write-Host "Optional recover test: unplug/replug cable, then press Enter" -ForegroundColor Yellow
    [void][Console]::ReadLine()
    [void](Send-Cmd "net wire recover")
    [void](Send-Cmd "net wire ping")
}
finally {
    $text = $script:sb.ToString()
    Set-Content -Path $transcriptFile -Value $text -Encoding UTF8
    if ($sp.IsOpen) { $sp.Close() }
}

$passPing = $text -match 'gem wire ping: PASS'
$passWrap = $text -match 'gem wire wrap: PASS'
$noReinit = $text -match 'no GEM reinit'
$linkUp = $text -match 'gem link:.*\bUP\b'
$verdict = if ($passPing -and $passWrap -and $linkUp) { "SOFTWARE_CAPTURE_OK — review peer log before PASS" } else { "NOT PASS" }

@(
    "# PN-WIRE-1 COM12 report",
    "",
    "**Run:** $runId",
    "**Verdict:** $verdict",
    "",
    "| Check | Result |",
    "|-------|--------|",
    "| link UP | $linkUp |",
    "| wire ping PASS | $passPing |",
    "| wire wrap PASS | $passWrap |",
    "| no GEM reinit string | $noReinit |",
    "",
    "Host peer log must show REQ OK + REP sent.",
    "Do not claim two-board PhaseNet seal from host peer.",
    "",
    "Transcript: transcript.txt"
) | Set-Content -Path $reportFile -Encoding UTF8

Write-Host ""
Write-Host "Wrote $transcriptFile"
Write-Host "Wrote $reportFile"
Write-Host "Verdict: $verdict"
