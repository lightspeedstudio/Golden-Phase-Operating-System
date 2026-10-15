# Live COM12 verification for real BootSessionRoot (PhaseHash v1) + PhaseIndex-
# backed filesystem operations on PZ7020-SL-C (kairos_zynq).
#
# Flow: open COM12 (115200 8N1) -> you press RESET -> capture the boot banner
# (includes the live BootSessionRoot line) until "gpos> " -> drive mkdir/write/
# cat/stat/ls/lock/unlock/rm through the real serial console -> save transcript.
#
# Drives the REAL interactive console over PS UART0 RX/TX. Every response is
# the board's; nothing here is faked, replayed, or scripted kernel-side.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts\capture_phase_index_verify_com12.ps1
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
$capFile = Join-Path $OutDir "com12_phase_index_verify_$stamp.txt"

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
        Write-Host "`n[OK] reached 'gpos> ' - kernel shell is up. Driving PhaseIndex-backed fs commands..."
        [void](Send-Cmd -Cmd "mkdir /docs")
        [void](Send-Cmd -Cmd "write /docs/a.txt hello-phaseindex")
        [void](Send-Cmd -Cmd "cat /docs/a.txt")
        [void](Send-Cmd -Cmd "stat /docs/a.txt")
        [void](Send-Cmd -Cmd "ls /docs")
        [void](Send-Cmd -Cmd "lock /docs/a.txt")
        [void](Send-Cmd -Cmd "stat /docs/a.txt")
        [void](Send-Cmd -Cmd "unlock /docs/a.txt")
        [void](Send-Cmd -Cmd "stat /docs/a.txt")
        [void](Send-Cmd -Cmd "rm /docs/a.txt")
        [void](Send-Cmd -Cmd "ls /docs")
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
