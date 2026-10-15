# P3 hardware test: run `usb keys` on the real PZ7020 over COM12.
#
# Assumes: board powered with the P3 image, USB keyboard in J10.
# Flow: open COM12 (DTR/RTS OFF) -> find the `gpos> ` prompt -> send `usb keys`
# -> the board enumerates, requests boot protocol, then polls the interrupt-IN
# endpoint for ~5s. >>> PRESS KEYS ON THE USB KEYBOARD during that window <<<
# -> capture the reports -> print transcript + verdict.

param(
    [string]$Port = "COM12",
    [int]$Baud = 115200,
    [int]$PromptWaitSec = 60,
    [int]$CmdTimeoutSec = 20   # 5s poll window + enumeration + margin
)

$ErrorActionPreference = "Stop"
$prompt = "gpos> "
$OutDir = "$PSScriptRoot\..\fpga\zynq\evidence\kairos_zynq_boot\usb_keys_test"
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$capFile = Join-Path $OutDir ("usb_keys_" + (Get-Date -Format "yyyyMMdd_HHmmss") + ".txt")

$sp = New-Object System.IO.Ports.SerialPort $Port, $Baud, ([System.IO.Ports.Parity]::None), 8, ([System.IO.Ports.StopBits]::One)
$sp.ReadTimeout = 300; $sp.NewLine = "`n"; $sp.DtrEnable = $false; $sp.RtsEnable = $false

$script:sb = New-Object System.Text.StringBuilder
function Read-Available { try { $c = $sp.ReadExisting(); if ($c) { [void]$script:sb.Append($c); Write-Host -NoNewline $c } } catch {} }
function Get-PromptCount { ([regex]::Matches($script:sb.ToString(), [regex]::Escape($prompt))).Count }
function Wait-ForPrompt { param([int]$Target,[int]$Sec)
    $deadline = (Get-Date).AddSeconds($Sec)
    while ((Get-Date) -lt $deadline) { Read-Available; if ((Get-PromptCount) -ge $Target) { return $true }; Start-Sleep -Milliseconds 40 }
    return $false
}

$verdict = "UNKNOWN"; $reply = ""
try {
    $sp.Open()
    Write-Host "[test] COM12 open. Finding prompt..."
    $sp.Write("`r"); Start-Sleep -Milliseconds 300; Read-Available
    if ((Get-PromptCount) -lt 1) { [void](Wait-ForPrompt -Target 1 -Sec $PromptWaitSec) }
    if ((Get-PromptCount) -lt 1) { $verdict = "NO-PROMPT (check power / cold boot / COM12)" }
    else {
        Write-Host "`n[test] shell up. Sending: usb keys"
        Write-Host "[test] >>> PRESS KEYS ON THE USB KEYBOARD NOW (for ~5 seconds) <<<`n"
        $before = Get-PromptCount; $preLen = $script:sb.Length
        $sp.Write("usb keys`r")
        $ok = Wait-ForPrompt -Target ($before + 1) -Sec $CmdTimeoutSec
        Start-Sleep -Milliseconds 300; Read-Available
        $reply = $script:sb.ToString().Substring($preLen)

        $reports = ([regex]::Matches($reply, 'report:')).Count
        if (-not $ok) { $verdict = "HANG (no prompt after 'usb keys')" }
        elseif ($reply -match 'unknown command') { $verdict = "STALE-IMAGE (old BIN; re-flash + cold boot)" }
        elseif ($reply -match 'no device connected') { $verdict = "NO-DEVICE (keyboard not in J10?)" }
        elseif ($reply -match 'no interrupt-IN endpoint') { $verdict = "NOT-HID-INPUT (no interrupt-IN endpoint found)" }
        elseif ($reports -gt 0) { $verdict = "PASS -- captured $reports key report(s) over USB" }
        elseif ($reply -match '0 report\(s\) captured') { $verdict = "ENUM-OK-NO-KEYS (endpoint polled but no keypress seen -- press keys during the window)" }
        else { $verdict = "UNCLEAR -- see transcript" }
    }
}
finally { if ($sp.IsOpen) { $sp.Close() }; $sp.Dispose() }

$script:sb.ToString() | Set-Content $capFile -Encoding UTF8
Write-Host "`n=================== USB KEYS TEST ==================="
Write-Host "VERDICT: $verdict"
Write-Host "transcript: $capFile"
