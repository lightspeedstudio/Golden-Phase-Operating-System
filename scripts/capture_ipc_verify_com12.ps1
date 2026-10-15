# Live COM12 verification for the real IPC and message-queue subsystem
# (kernel::ipc) on PZ7020-SL-C (kairos_zynq), Subsystem 5 (ipc) Gate E.
#
# Flow: open COM12 (115200 8N1) -> you press RESET -> capture the boot banner
# until "gpos> " -> baseline `captable`/`ipc` (nothing minted yet) ->
# `ipccreate 4` (real captable object+capability mint, real ipc queue) ->
# `captable`/`ipc` (cross-check: live_objects/live_capabilities/queues_live
# all +1) -> `ipcsend ... hello-ipc` -> `ipcrecv ...` (immediate, non-empty
# queue) -> `ipcrecv ...` again on the now-empty queue (registers a REAL
# block in kernel::sched2 -- `sched2` must show blocked=1 right after) ->
# `ipcsend ... wake-me` (a real send on the same queue must call
# Sched2::wake_matching and wake the caller -- `sched2` must show blocked=0
# again) -> `ipcrecv ...` drains the "wake-me" message just sent -> fill the
# capacity-4 queue and send a 5th message to prove a REAL QueueFull rejection
# -> drain all 4 in strict FIFO order -> `ipcclose ...` (real captable
# destroy, real ipc queue destroy) -> final `captable`/`ipc` (back to
# baseline).
#
# Drives the REAL interactive console over PS UART0 RX/TX. Every response is
# the board's; nothing here is faked, replayed, or scripted kernel-side.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts\capture_ipc_verify_com12.ps1
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
$capFile = Join-Path $OutDir "com12_ipc_verify_$stamp.txt"

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

# Sends $Cmd, waits for the next prompt, and returns the board's own response
# text (the buffer delta since just before this command was sent) so callers
# can parse real values (e.g. the "handle S:G" a real `ipccreate` mints) out
# of what the board actually printed.
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
        Write-Host "`n[OK] reached 'gpos> ' - kernel shell is up. Driving ipc-backed commands..."

        Write-Host "`n=== baseline captable stats (nothing minted yet) ==="
        [void](Send-Cmd -Cmd "captable")

        Write-Host "`n=== baseline ipc stats (no queues yet) ==="
        [void](Send-Cmd -Cmd "ipc")

        Write-Host "`n=== create a real bounded queue (capacity 4): real captable object+capability mint ==="
        $createOut = Send-Cmd -Cmd "ipccreate 4"
        $m = [regex]::Match($createOut, "handle (\d+):(\d+)")
        if (-not $m.Success) {
            Write-Host "`n[FAIL] could not parse a minted handle out of 'ipccreate 4' response"
            throw "ipccreate did not return a handle"
        }
        $slot = $m.Groups[1].Value
        $gen = $m.Groups[2].Value
        Write-Host "`n[OK] minted real queue handle $slot`:$gen"

        Write-Host "`n=== cross-check: captable live_objects/live_capabilities must be +1 ==="
        [void](Send-Cmd -Cmd "captable")

        Write-Host "`n=== cross-check: ipc queues_live/queues_created must be +1 ==="
        [void](Send-Cmd -Cmd "ipc")

        Write-Host "`n=== send one real message on the real queue ==="
        [void](Send-Cmd -Cmd "ipcsend $slot $gen hello-ipc")

        Write-Host "`n=== ipc sent counter must be +1 ==="
        [void](Send-Cmd -Cmd "ipc")

        Write-Host "`n=== baseline sched2 stats before any block (blocked must be 0) ==="
        [void](Send-Cmd -Cmd "sched2")

        Write-Host "`n=== recv the one queued message: must succeed immediately (queue was non-empty) ==="
        [void](Send-Cmd -Cmd "ipcrecv $slot $gen")

        Write-Host "`n=== recv again on the now-EMPTY queue: must report 'queue empty' and register a REAL block ==="
        [void](Send-Cmd -Cmd "ipcrecv $slot $gen")

        Write-Host "`n=== sched2 must now show blocked=1 for the shell's own pid (real WaitReason::Ipc registration) ==="
        [void](Send-Cmd -Cmd "sched2")

        Write-Host "`n=== send again on the same queue: must call Sched2::wake_matching and wake the blocked caller ==="
        [void](Send-Cmd -Cmd "ipcsend $slot $gen wake-me")

        Write-Host "`n=== sched2 must now show blocked=0 again (the real send really woke it) ==="
        [void](Send-Cmd -Cmd "sched2")

        Write-Host "`n=== drain the message the wake-triggering send just enqueued ==="
        [void](Send-Cmd -Cmd "ipcrecv $slot $gen")

        Write-Host "`n=== fill the capacity-4 queue with 4 real messages, in order ==="
        [void](Send-Cmd -Cmd "ipcsend $slot $gen msg-a")
        [void](Send-Cmd -Cmd "ipcsend $slot $gen msg-b")
        [void](Send-Cmd -Cmd "ipcsend $slot $gen msg-c")
        [void](Send-Cmd -Cmd "ipcsend $slot $gen msg-d")

        Write-Host "`n=== a 5th send on the now-full queue must be REJECTED (real QueueFull, send never blocks) ==="
        [void](Send-Cmd -Cmd "ipcsend $slot $gen msg-e")

        Write-Host "`n=== ipc send_rejected_full must be +1 ==="
        [void](Send-Cmd -Cmd "ipc")

        Write-Host "`n=== drain all 4 queued messages: must come back in strict FIFO order a,b,c,d ==="
        [void](Send-Cmd -Cmd "ipcrecv $slot $gen")
        [void](Send-Cmd -Cmd "ipcrecv $slot $gen")
        [void](Send-Cmd -Cmd "ipcrecv $slot $gen")
        [void](Send-Cmd -Cmd "ipcrecv $slot $gen")

        Write-Host "`n=== destroy the real queue: real captable object destroy + real ipc queue destroy ==="
        [void](Send-Cmd -Cmd "ipcclose $slot $gen")

        Write-Host "`n=== final captable stats: live_objects/live_capabilities back to baseline, closes +1 ==="
        [void](Send-Cmd -Cmd "captable")

        Write-Host "`n=== final ipc stats: queues_live back to 0, queues_destroyed +1 ==="
        [void](Send-Cmd -Cmd "ipc")

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
