# Live COM12 verification for the real scheduler and timer-queue subsystem
# (kernel::sched2) on PZ7020-SL-C (kairos_zynq), Subsystem 4 (sched2) Gate E.
#
# Flow: open COM12 (115200 8N1) -> you press RESET -> capture the boot banner
# until "gpos> " -> baseline `sched2` (elapsed_tick already moving, driven by
# the real private-timer PPI 29 through irqreg's proven registry; nothing
# registered yet) -> `irqreg` (cross-check: priv_timer_29's real count) ->
# `sleep 200` (blocks the shell's own pid in sched2 for 200 real sched2 ticks
# = ~1s at the 5ms/tick period armed at boot) -> `sched2` immediately after
# (must show blocked=1, timers_armed=1, before the real deadline) -> wait
# ~1.5s of real wall time -> `sched2` again (must show blocked=0, ready=1,
# timers_fired=1 -- the real hardware timer actually woke it) -> `irqreg`
# again (priv_timer_29 must have advanced by close to the real elapsed_tick
# delta, cross-checking sched2's time base against irqreg's own counter).
#
# Drives the REAL interactive console over PS UART0 RX/TX. Every response is
# the board's; nothing here is faked, replayed, or scripted kernel-side.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts\capture_sched2_verify_com12.ps1
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
$capFile = Join-Path $OutDir "com12_sched2_verify_$stamp.txt"

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
    Write-Host "`n>>> sending: $Cmd"
    $sp.Write("$Cmd`r")
    $ok = Wait-ForPrompt -Target ($before + 1) -TimeoutSec $TimeoutSec
    if (-not $ok) {
        Write-Host "`n[WARN] no new prompt after '$Cmd' within $TimeoutSec sec"
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
        Write-Host "`n[OK] reached 'gpos> ' - kernel shell is up. Driving sched2-backed commands..."

        Write-Host "`n=== baseline sched2 stats (right after boot; nothing registered yet) ==="
        [void](Send-Cmd -Cmd "sched2")

        Write-Host "`n=== baseline irqreg stats (cross-check: priv_timer_29 is sched2's real tick source) ==="
        [void](Send-Cmd -Cmd "irqreg")

        Write-Host "`n=== block the shell's own pid in sched2 for 200 real ticks (~1s @ 5ms/tick) ==="
        [void](Send-Cmd -Cmd "sleep 200")

        Write-Host "`n=== immediately re-check: must show blocked=1, timers_armed=1 (before the real deadline) ==="
        [void](Send-Cmd -Cmd "sched2")

        Write-Host "`n=== wait ~1.5s of real wall time for the real hardware timer to fire ==="
        Start-Sleep -Milliseconds 1500

        Write-Host "`n=== final sched2 stats: must show blocked=0, ready=1, timers_fired=1 ==="
        [void](Send-Cmd -Cmd "sched2")

        Write-Host "`n=== final irqreg stats: priv_timer_29 must have advanced (cross-check vs sched2's elapsed_tick) ==="
        [void](Send-Cmd -Cmd "irqreg")

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
