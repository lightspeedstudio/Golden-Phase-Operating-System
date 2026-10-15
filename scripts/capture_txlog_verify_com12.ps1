# Live COM12 verification for the real multi-op transaction log
# (kernel::txlog) on PZ7020-SL-C (kairos_zynq), Subsystem 7 (txlog) Gate E.
#
# Flow: open COM12 (115200 8N1) -> you press RESET -> capture the boot banner
# until "gpos> " -> baseline `txlog` (zero counters) -> `txlog demo` (a real
# 3-op WriteFile+Mkdir+Rm transaction that commits atomically through the
# real kernel::txlog::Txlog::run_transaction, verified against PhaseFs by
# verify_record/verify_intent) -> `cat` the new file back exactly -> `ls /`
# to show the stale file is genuinely gone and the new dir genuinely exists
# -> `txlog` stats after (records+1, committed+1) -> `txlog fault` (a real
# 2-op transaction with op1 never attempted because of an injected
# InjectedFault::StopAfterOp(1); op0's real overwrite is genuinely undone by
# rollback) -> `cat` the target file back to prove it is really restored to
# its pre-transaction content, not left half-written -> `ls /` to show the
# never-attempted mkdir genuinely never appeared -> final `txlog` stats
# (records+1 more, rolled_back+1).
#
# NOTE: this board's shell halts the whole kernel on ANY command that
# returns an error (e.g. `cat`/`ls` of a path that does not exist) --
# pre-existing, unrelated to `txlog` (`kernel_main.rs`'s `run_serial_shell`
# propagates every `ShellError` with `?`, and the caller then halts). This
# script therefore only ever `cat`s paths that are known to exist, and uses
# `ls /` (always exists) to observe absence, never a direct `cat`/`ls` of a
# missing path.
#
# Drives the REAL interactive console over PS UART0 RX/TX. Every response is
# the board's; nothing here is faked, replayed, or scripted kernel-side.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts\capture_txlog_verify_com12.ps1
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
$capFile = Join-Path $OutDir "com12_txlog_verify_$stamp.txt"

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
        Write-Host "`n[OK] reached 'gpos> ' - kernel shell is up. Driving txlog-backed commands..."

        Write-Host "`n=== baseline txlog stats (no transactions run yet) ==="
        [void](Send-Cmd -Cmd "txlog")

        Write-Host "`n=== run a real 3-op transaction (write + mkdir + rm), must commit atomically ==="
        [void](Send-Cmd -Cmd "txlog demo")

        Write-Host "`n=== cat the file the transaction wrote: must return the exact committed content ==="
        [void](Send-Cmd -Cmd "cat /txlog_demo.txt")

        Write-Host "`n=== ls / : the stale file must be genuinely GONE, the new dir must genuinely be present ==="
        [void](Send-Cmd -Cmd "ls /")

        Write-Host "`n=== txlog stats after demo: records +1, committed +1 ==="
        [void](Send-Cmd -Cmd "txlog")

        Write-Host "`n=== run a real 2-op transaction with op1 (mkdir) never attempted -- op0's real overwrite must be rolled back ==="
        [void](Send-Cmd -Cmd "txlog fault")

        Write-Host "`n=== cat the target file: must show the ORIGINAL content, proving the overwrite was genuinely undone ==="
        [void](Send-Cmd -Cmd "cat /txlog_fault_target.txt")

        Write-Host "`n=== ls / : the never-attempted mkdir must NOT appear ==="
        [void](Send-Cmd -Cmd "ls /")

        Write-Host "`n=== final txlog stats: records +1 more, rolled_back +1, faults_injected +1 ==="
        [void](Send-Cmd -Cmd "txlog")

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
