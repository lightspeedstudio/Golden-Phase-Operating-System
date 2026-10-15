# Live COM12 verification for the "all systems" kairos_zynq image: every
# non-craft/mission feature built in at once (pmm, captable, irqreg, sched2,
# ipc, extmap, txlog, netsess, bm-net, bm-engineering-tools, ssh-bm, usb,
# g64-research, phaseguard-research, and every phase-math backend).
#
# This is a smoke test, not a per-subsystem Gate E: it boots the board and
# confirms every one of the 8 kernel subsystems' shell commands is present
# and responds against the real kernel, then runs one real end-to-end demo
# (netsess demo, which itself exercises captable + sched2 underneath) to
# prove genuine cross-subsystem behavior, not just command presence.
#
# Drives the REAL interactive console over PS UART0 RX/TX. Every response is
# the board's; nothing here is faked, replayed, or scripted kernel-side.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts\capture_allsys_verify_com12.ps1

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
$capFile = Join-Path $OutDir "com12_allsys_verify_$stamp.txt"

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
        Write-Host "`n[OK] reached 'gpos> ' - kernel shell is up. Probing all 8 subsystems..."

        Write-Host "`n=== 1/8 pmm: stats + selftest ==="
        [void](Send-Cmd -Cmd "pmm")
        [void](Send-Cmd -Cmd "pmm selftest")
        [void](Send-Cmd -Cmd "pmm")

        Write-Host "`n=== 2/8 captable: stats ==="
        [void](Send-Cmd -Cmd "captable")

        Write-Host "`n=== 3/8 irqreg: stats ==="
        [void](Send-Cmd -Cmd "irqreg")

        Write-Host "`n=== 4/8 sched2: stats ==="
        [void](Send-Cmd -Cmd "sched2")

        Write-Host "`n=== 5/8 ipc: stats ==="
        [void](Send-Cmd -Cmd "ipc")

        Write-Host "`n=== 6/8 extmap: stats ==="
        [void](Send-Cmd -Cmd "extmap")

        Write-Host "`n=== 7/8 txlog: stats + demo + stats ==="
        [void](Send-Cmd -Cmd "txlog")
        [void](Send-Cmd -Cmd "txlog demo")
        [void](Send-Cmd -Cmd "txlog")

        Write-Host "`n=== 8/8 netsess: baseline stats -> real demo (captable+sched2 backed session, replay accept, descriptor ring, RTT) -> stats after ==="
        [void](Send-Cmd -Cmd "netsess")
        [void](Send-Cmd -Cmd "netsess demo")
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
