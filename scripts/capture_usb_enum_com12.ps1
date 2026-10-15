# Capture the `usb enum [v5 phy-reset+ulpi-wu]` run on the real PZ7020 over COM12.
#
# Flow: open COM12 (115200 8N1, DTR/RTS off) -> wait for the `gpos> ` prompt (you
# cold-boot the board) -> prompt appears, you PLUG THE USB DEVICE into J10 ->
# after -SettleSec the script sends `usb enum` -> captures the full multi-line
# report incl. every `#DBG_fe0da0#` NDJSON line -> saves the transcript.
#
# This drives the REAL console over PS UART0. Nothing on the kernel side is faked.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts\capture_usb_enum_com12.ps1
#   powershell ... -Port COM12 -BootTimeoutSec 90 -SettleSec 25 -CmdTimeoutSec 40

param(
    [string]$Port = "COM12",
    [int]$Baud = 115200,
    [int]$BootTimeoutSec = 90,
    [int]$SettleSec = 25,       # window to plug the USB device in after boot
    [int]$CmdTimeoutSec = 40,   # usb enum has bus delays + ULPI viewport spins
    [string]$OutDir = "$PSScriptRoot\..\fpga\zynq\evidence\kairos_zynq_boot\usb_ulpi_diag"
)

$ErrorActionPreference = "Stop"
$prompt = "gpos> "

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$stamp = Get-Date -Format "yyyyMMdd_HHmmss"
$capFile = Join-Path $OutDir "usb_enum_v4_$stamp.txt"

$sp = New-Object System.IO.Ports.SerialPort $Port, $Baud, ([System.IO.Ports.Parity]::None), 8, ([System.IO.Ports.StopBits]::One)
$sp.ReadTimeout = 300
$sp.NewLine = "`n"
$sp.DtrEnable = $false
$sp.RtsEnable = $false

$script:sb = New-Object System.Text.StringBuilder

function Read-Available {
    try {
        $chunk = $sp.ReadExisting()
        if ($chunk) { [void]$script:sb.Append($chunk); Write-Host -NoNewline $chunk }
    } catch {}
}
function Get-PromptCount { ([regex]::Matches($script:sb.ToString(), [regex]::Escape($prompt))).Count }
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

try {
    $sp.Open()
    Write-Host "LISTENING $Port @ $Baud - COLD POWER-CYCLE the board now (up to $BootTimeoutSec sec for 'gpos> ')..."
    $booted = Wait-ForPrompt -Target 1 -TimeoutSec $BootTimeoutSec
    if (-not $booted) {
        Write-Host "`n[FAIL] no 'gpos> ' within $BootTimeoutSec sec - board did not reach the shell."
    }
    else {
        Write-Host "`n[OK] shell up. >>> PLUG THE USB DEVICE INTO J10 NOW <<< ($SettleSec sec until 'usb enum')"
        $settleEnd = (Get-Date).AddSeconds($SettleSec)
        while ((Get-Date) -lt $settleEnd) { Read-Available; Start-Sleep -Milliseconds 60 }

        $before = Get-PromptCount
        Write-Host "`n>>> sending: usb enum"
        $sp.Write("usb enum`r")
        $ok = Wait-ForPrompt -Target ($before + 1) -TimeoutSec $CmdTimeoutSec
        if (-not $ok) { Write-Host "`n[WARN] no new prompt after 'usb enum' within $CmdTimeoutSec sec" }
        $drainEnd = (Get-Date).AddSeconds(1)
        while ((Get-Date) -lt $drainEnd) { Read-Available; Start-Sleep -Milliseconds 40 }
    }
}
finally {
    if ($sp.IsOpen) { $sp.Close() }
    $sp.Dispose()
}

$full = $script:sb.ToString()
$full | Set-Content $capFile -Encoding UTF8

# Quick on-screen checks.
$sawV4  = $full -match '\[v5 phy-reset\+ulpi-wu\]'
$dbgCnt = ([regex]::Matches($full, '#DBG_fe0da0#')).Count
Write-Host "`n`nSaved transcript: $capFile"
Write-Host ("version banner [v5 phy-reset+ulpi-wu]: {0}" -f $(if ($sawV4) { 'YES (new image running)' } else { 'NO - STALE IMAGE, re-flash + cold boot' }))
Write-Host "#DBG_fe0da0# lines captured: $dbgCnt"
Write-Host "`nCAPTURE_FILE=$capFile"
