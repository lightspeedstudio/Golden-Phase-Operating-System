# Live COM12 verification for the real network session table + descriptor
# ring (kernel::netsess) on PZ7020-SL-C (kairos_zynq), Subsystem 8 (netsess,
# the final subsystem) Gate E.
#
# Flow: open COM12 (115200 8N1) -> you press RESET -> capture the boot banner
# until "gpos> " -> baseline `netsess` (zero counters) -> `netsess demo` (a
# real session open, an in-order + a reordered-inside-window replay accept,
# one descriptor push/drain/complete round trip, and two real RTT samples
# through kernel::netsess::Netsess) -> `netsess` stats after (sessions_opened
# +1, replay_accepted +2, descriptors_pushed/completed +1) -> `netsess fault`
# (a real duplicate-sequence rejection, a real fail_descriptor quarantine,
# a second fail on the same now-quarantined slot rejected, and a real
# descriptor-ring-full backpressure after RING_CAPACITY pushes) -> final
# `netsess` stats (replay_rejected +1, descriptors_failed +1,
# descriptors_rejected_full +1).
#
# Drives the REAL interactive console over PS UART0 RX/TX. Every response is
# the board's; nothing here is faked, replayed, or scripted kernel-side.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts\capture_netsess_verify_com12.ps1
#   powershell ... -Port COM12 -BootTimeoutSec 60 -CmdTimeoutSec 12

param(
    [string]$Port = "COM12",
    [int]$Baud = 115200,
    [int]$BootTimeoutSec = 60,
    [int]$CmdTimeoutSec = 20,
    [string]$OutDir = "$PSScriptRoot\..\fpga\zynq\evidence\kairos_zynq_boot"
)

$ErrorActionPreference = "Stop"
$prompt = "gpos> "

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$stamp = Get-Date -Format "yyyyMMdd_HHmmss"
$capFile = Join-Path $OutDir "com12_netsess_verify_$stamp.txt"

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
    param([string]$Cmd, [int]$TimeoutSec = 0)
    if ($TimeoutSec -le 0) { $TimeoutSec = $CmdTimeoutSec }
    $before = Get-PromptCount
    $beforeLen = $script:sb.Length
    Write-Host "`n>>> sending: $Cmd"
    $sp.Write("$Cmd`r")
    $ok = Wait-ForPrompt -Target ($before + 1) -TimeoutSec $TimeoutSec
    if (-not $ok) {
        Write-Host "`n[WARN] no new prompt after '$Cmd' within $TimeoutSec sec"
    }
    return $script:sb.ToString().Substring($beforeLen)
}

try {
    $sp.Open()
    Write-Host "LISTENING $Port @ $Baud - press RESET on the board now (up to $BootTimeoutSec sec for 'gpos> ')..."

    $booted = Wait-ForPrompt -Target 1 -TimeoutSec $BootTimeoutSec
    if (-not $booted) {
        Write-Host "`n[FAIL] no 'gpos> ' prompt within $BootTimeoutSec sec - board did not reach the shell."
    }
    else {
        Write-Host "`n[OK] reached 'gpos> ' - kernel shell is up. Driving netsess-backed commands..."

        Write-Host "`n=== baseline netsess stats (no sessions opened yet) ==="
        [void](Send-Cmd -Cmd "netsess")

        Write-Host "`n=== real session open + in-order/reordered replay accept + descriptor push/drain/complete + 2 RTT samples ==="
        [void](Send-Cmd -Cmd "netsess demo")

        Write-Host "`n=== netsess stats after demo: sessions_opened +1, replay_accepted +2, descriptors_pushed/completed +1 ==="
        [void](Send-Cmd -Cmd "netsess")

        Write-Host "`n=== real duplicate-sequence rejection + fail_descriptor quarantine + refail-on-quarantined rejection + ring-full backpressure ==="
        [void](Send-Cmd -Cmd "netsess fault")

        Write-Host "`n=== final netsess stats: replay_rejected +1, descriptors_failed +1, descriptors_rejected_full +1 ==="
        [void](Send-Cmd -Cmd "netsess")

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
