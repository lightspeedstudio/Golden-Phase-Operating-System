# Live COM12 verification for the real generational object/capability table
# (kernel::captable) on PZ7020-SL-C (kairos_zynq), Subsystem 2 (captable) Gate E.
#
# Flow: open COM12 (115200 8N1) -> you press RESET -> capture the boot banner
# until "gpos> " -> baseline `captable` stats -> write a real file -> `open`
# it (mints a real capability; the handle "slot:generation" is parsed live
# out of the board's own printed response) -> `captable` stats again (opens
# counter must have moved) -> `close <slot> <generation>` (revokes it) ->
# `captable` stats again (closes counter must have moved) -> attempt the
# exact same `close <slot> <generation>` a second time (must be rejected,
# not silently accepted) -> attempt `open` on a nonexistent path (must fail,
# not mint a capability for nothing).
#
# Drives the REAL interactive console over PS UART0 RX/TX. Every response is
# the board's; nothing here is faked, replayed, or scripted kernel-side —
# the close command's slot/generation arguments are extracted from the
# board's own live UART output, not hardcoded.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts\capture_captable_verify_com12.ps1
#   powershell ... -Port COM12 -BootTimeoutSec 60 -CmdTimeoutSec 12

param(
    [string]$Port = "COM12",
    [int]$Baud = 115200,
    [int]$BootTimeoutSec = 60,
    [int]$CmdTimeoutSec = 12,
    [string]$OutDir = "$PSScriptRoot\..\fpga\zynq\evidence\kairos_zynq_boot"
)

$ErrorActionPreference = "Stop"
$prompt = "gpos> "

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$stamp = Get-Date -Format "yyyyMMdd_HHmmss"
$capFile = Join-Path $OutDir "com12_captable_verify_$stamp.txt"

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
    $t = $script:sb.ToString()
    return ([regex]::Matches($t, [regex]::Escape($prompt))).Count
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
    Write-Host "`n>>> sending: $Cmd"
    $sp.Write("$Cmd`r")
    $ok = Wait-ForPrompt -Target ($before + 1) -TimeoutSec $CmdTimeoutSec
    if (-not $ok) {
        Write-Host "`n[WARN] no new prompt after '$Cmd' within $CmdTimeoutSec sec"
    }
    return $ok
}

try {
    $sp.Open()
    Write-Host "LISTENING $Port @ $Baud - press RESET on the board now (up to $BootTimeoutSec sec for 'gpos> ')..."

    $booted = Wait-ForPrompt -Target 1 -TimeoutSec $BootTimeoutSec
    if (-not $booted) {
        Write-Host "`n[FAIL] no 'gpos> ' prompt within $BootTimeoutSec sec - board did not reach the shell."
    }
    else {
        Write-Host "`n[OK] reached 'gpos> ' - kernel shell is up. Driving captable-backed commands..."

        Write-Host "`n=== baseline captable stats ==="
        [void](Send-Cmd -Cmd "captable")

        Write-Host "`n=== create a real file to open ==="
        [void](Send-Cmd -Cmd "write /captest.txt hello-captable")

        Write-Host "`n=== FS_OPEN: mint a real capability (read+write) ==="
        [void](Send-Cmd -Cmd "open /captest.txt rw")

        Write-Host "`n=== captable stats after open (opens counter must have moved) ==="
        [void](Send-Cmd -Cmd "captable")

        $text = $script:sb.ToString()
        $m = [regex]::Match($text, "handle (\d+):(\d+)")
        if ($m.Success) {
            $slot = $m.Groups[1].Value
            $gen = $m.Groups[2].Value
            Write-Host "`n[OK] parsed minted handle from live board output: slot=$slot generation=$gen"

            Write-Host "`n=== FS_CLOSE: revoke the real handle ==="
            [void](Send-Cmd -Cmd "close $slot $gen")

            Write-Host "`n=== captable stats after close (closes counter must have moved) ==="
            [void](Send-Cmd -Cmd "captable")

            Write-Host "`n=== double-close the same handle (must be rejected, not silently accepted) ==="
            [void](Send-Cmd -Cmd "close $slot $gen")
        }
        else {
            Write-Host "`n[WARN] could not parse a 'handle SLOT:GENERATION' from the board's 'open' response - skipping close/double-close steps"
        }

        Write-Host "`n=== FS_OPEN on a nonexistent path (must fail, not mint a capability for nothing) ==="
        [void](Send-Cmd -Cmd "open /does-not-exist.txt r")

        $drainEnd = (Get-Date).AddSeconds(1)
        while ((Get-Date) -lt $drainEnd) { Read-Available; Start-Sleep -Milliseconds 40 }
    }
}
finally {
    if ($sp.IsOpen) { $sp.Close() }
    $sp.Dispose()
}

$script:sb.ToString() | Set-Content $capFile -Encoding UTF8
Write-Host "`n`nSaved transcript: $capFile"
