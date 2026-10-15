# P2 hardware test: run `usb list` on the real PZ7020 over COM12 and judge it.
#
# Assumes: board is powered with the P2 image, USB keyboard already in J10.
# Flow: open COM12 (DTR/RTS OFF so we don't reset the board) -> find the `gpos> `
# prompt (nudge with Enter; if none, wait for a boot banner) -> send `usb list`
# -> capture the reply -> print the transcript + a PASS/FAIL verdict.
#
# Drives the REAL console over PS UART0. Nothing is faked on the kernel side.

param(
    [string]$Port = "COM12",
    [int]$Baud = 115200,
    [int]$PromptWaitSec = 60,   # allow a cold boot to finish
    [int]$CmdTimeoutSec = 45    # usb list = bus delays + enumeration
)

$ErrorActionPreference = "Stop"
$prompt = "gpos> "
$OutDir = "$PSScriptRoot\..\fpga\zynq\evidence\kairos_zynq_boot\usb_list_test"
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$stamp = Get-Date -Format "yyyyMMdd_HHmmss"
$capFile = Join-Path $OutDir "usb_list_$stamp.txt"

$sp = New-Object System.IO.Ports.SerialPort $Port, $Baud, ([System.IO.Ports.Parity]::None), 8, ([System.IO.Ports.StopBits]::One)
$sp.ReadTimeout = 300
$sp.NewLine = "`n"
$sp.DtrEnable = $false   # CH340 wires DTR/RTS to reset -- keep them low
$sp.RtsEnable = $false

$script:sb = New-Object System.Text.StringBuilder
function Read-Available {
    try { $c = $sp.ReadExisting(); if ($c) { [void]$script:sb.Append($c); Write-Host -NoNewline $c } } catch {}
}
function Get-PromptCount { ([regex]::Matches($script:sb.ToString(), [regex]::Escape($prompt))).Count }
function Wait-ForPrompt { param([int]$Target,[int]$Sec)
    $deadline = (Get-Date).AddSeconds($Sec)
    while ((Get-Date) -lt $deadline) { Read-Available; if ((Get-PromptCount) -ge $Target) { return $true }; Start-Sleep -Milliseconds 40 }
    return $false
}

$verdict = "UNKNOWN"; $reply = ""
try {
    $sp.Open()
    Write-Host "[test] COM12 open. Looking for the shell prompt..."
    # If the board is already booted & idle it won't emit anything -- nudge it.
    $sp.Write("`r"); Start-Sleep -Milliseconds 300; Read-Available
    $havePrompt = (Get-PromptCount) -ge 1
    if (-not $havePrompt) {
        Write-Host "[test] no prompt yet -- waiting up to $PromptWaitSec s for boot (cold-boot now if you haven't)..."
        $havePrompt = Wait-ForPrompt -Target 1 -Sec $PromptWaitSec
    }
    if (-not $havePrompt) {
        $verdict = "NO-PROMPT (board not reaching gpos> -- check power / cold boot / COM12)"
    }
    else {
        Write-Host "`n[test] shell is up. Sending: usb list"
        $before = Get-PromptCount
        $preLen = $script:sb.Length
        $sp.Write("usb list`r")
        $ok = Wait-ForPrompt -Target ($before + 1) -Sec $CmdTimeoutSec
        Start-Sleep -Milliseconds 300; Read-Available
        $reply = $script:sb.ToString().Substring($preLen)

        if (-not $ok) { $verdict = "HANG (no prompt after 'usb list' in $CmdTimeoutSec s)" }
        elseif ($reply -match 'unknown command') { $verdict = "STALE-IMAGE ('usb list' unknown -- old BIN booted; re-flash + cold boot)" }
        elseif ($reply -match 'usb1:\s*([0-9a-fA-F]{4}):([0-9a-fA-F]{4}).*class=(\S+)') {
            $verdict = "PASS -- enumerated $($Matches[1]):$($Matches[2]) class=$($Matches[3])"
        }
        elseif ($reply -match 'no device connected') { $verdict = "NO-DEVICE (keyboard not detected -- is it in J10? try re-seating)" }
        elseif ($reply -match 'control transfer (halted|timeout)') { $verdict = "ENUM-ERROR ($($Matches[0])) -- transfer failed mid-enumeration" }
        elseif ($reply -match 'requires an arm usb') { $verdict = "WRONG-BUILD (usb feature not in this image)" }
        else { $verdict = "UNCLEAR -- see transcript" }
    }
}
finally { if ($sp.IsOpen) { $sp.Close() }; $sp.Dispose() }

$script:sb.ToString() | Set-Content $capFile -Encoding UTF8
Write-Host "`n=================== USB LIST TEST ==================="
Write-Host "VERDICT: $verdict"
Write-Host "transcript: $capFile"
Write-Host "---- reply ----"
Write-Host $reply
