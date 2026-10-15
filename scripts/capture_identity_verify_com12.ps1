# Live COM12 verification for kernel::identity on PZ7020-SL-C (Gate 13).
#
# Flow: open COM12 (115200 8N1) -> you press RESET -> wait for "gpos> "
# -> passwd --status (unprovisioned) -> passwd (provision) -> passwd --status
# (active) -> login -> mod -l -> mod shell (refused) -> mod fs.rm (ticket)
# -> logout -> mod fs.rm (no session) -> login with a wrong password (denied).
#
# Drives the REAL interactive console over PS UART0. Secret lines use the
# real no-echo UART reader. Argon2id on the A9 can take several seconds.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts\capture_identity_verify_com12.ps1

param(
    [string]$Port = "COM12",
    [int]$Baud = 115200,
    [int]$BootTimeoutSec = 90,
    [int]$CmdTimeoutSec = 20,
    [int]$HashTimeoutSec = 90,
    [string]$Password = "correct horse battery staple one",
    [string]$OutDir = "$PSScriptRoot\..\fpga\zynq\evidence\kairos_zynq_boot"
)

$ErrorActionPreference = "Stop"
$prompt = "gpos> "

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$stamp = Get-Date -Format "yyyyMMdd_HHmmss"
$capFile = Join-Path $OutDir "com12_identity_verify_$stamp.txt"

$sp = New-Object System.IO.Ports.SerialPort $Port, $Baud, ([System.IO.Ports.Parity]::None), 8, ([System.IO.Ports.StopBits]::One)
$sp.ReadTimeout = 300
$sp.NewLine = "`n"
# Do not assert DTR/RTS: CH340 DTR can reset the board, and RTS can stall
# Zynq UART TX if the adapter presents it as flow control.
$sp.DtrEnable = $false
$sp.RtsEnable = $false

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
    # Real shell prompt is at the start of a line. The boot splash also
    # contains the substring "gpos> " ("type help at gpos>"), which must
    # not count as the kernel being ready.
    return ([regex]::Matches($script:sb.ToString(), '(?m)^gpos> ')).Count
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

function Wait-ForNewText {
    param([string]$Needle, [int]$AfterLength, [int]$TimeoutSec)
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while ((Get-Date) -lt $deadline) {
        Read-Available
        $t = $script:sb.ToString()
        if ($t.Length -gt $AfterLength -and $t.Substring($AfterLength).Contains($Needle)) {
            return $true
        }
        Start-Sleep -Milliseconds 40
    }
    return $false
}

function Send-Secret {
    param(
        [string]$PromptNeedle,
        [string]$Secret,
        [int]$PromptTimeoutSec = 20,
        [int]$AfterLength = -1
    )
    if ($AfterLength -lt 0) { $AfterLength = $script:sb.Length }
    $ok = Wait-ForNewText -Needle $PromptNeedle -AfterLength $AfterLength -TimeoutSec $PromptTimeoutSec
    if (-not $ok) {
        Write-Host "`n[WARN] did not see prompt '$PromptNeedle'"
        return
    }
    Write-Host "`n>>> sending secret ($($Secret.Length) chars, not echoed here)"
    $sp.Write("$Secret`r")
}

function Send-Line {
    param([string]$Cmd)
    Write-Host "`n>>> sending: $Cmd"
    $sp.Write("$Cmd`r")
}

function Send-Cmd {
    param([string]$Cmd, [int]$TimeoutSec = 0)
    if ($TimeoutSec -le 0) { $TimeoutSec = $CmdTimeoutSec }
    $before = Get-PromptCount
    Send-Line -Cmd $Cmd
    $ok = Wait-ForPrompt -Target ($before + 1) -TimeoutSec $TimeoutSec
    if (-not $ok) {
        Write-Host "`n[WARN] no new prompt after '$Cmd' within $TimeoutSec sec"
    }
}

try {
    $sp.Open()
    Write-Host "LISTENING $Port @ $Baud - press RESET on the board now (up to $BootTimeoutSec sec for 'gpos> ')..."

    $booted = Wait-ForPrompt -Target 1 -TimeoutSec $BootTimeoutSec
    if (-not $booted) {
        Write-Host "`n[FAIL] no 'gpos> ' prompt within $BootTimeoutSec sec - board did not reach the shell."
    }
    else {
        Write-Host "`n[OK] reached 'gpos> '. Driving identity commands..."

        Send-Cmd -Cmd "passwd --status"
        $statusText = $script:sb.ToString()
        $needsProvision = $statusText -match 'unprovisioned'

        if ($needsProvision) {
            $beforePasswd = Get-PromptCount
            $markPasswd = $script:sb.Length
            Send-Line -Cmd "passwd"
            $sawNew = Wait-ForNewText -Needle "new root password:" -AfterLength $markPasswd -TimeoutSec 20
            if (-not $sawNew) {
                Write-Host "`n[WARN] did not see 'new root password:' - sending password anyway"
            }
            Write-Host "`n>>> sending new-password secret"
            $sp.Write("$Password`r")
            $sawConfirm = Wait-ForNewText -Needle "confirm new root password:" -AfterLength $markPasswd -TimeoutSec 20
            if (-not $sawConfirm) {
                Write-Host "`n[WARN] did not see confirm prompt - sending password anyway"
            }
            Write-Host "`n>>> sending confirm-password secret"
            $sp.Write("$Password`r")
            if (-not (Wait-ForPrompt -Target ($beforePasswd + 1) -TimeoutSec $HashTimeoutSec)) {
                Write-Host "`n[WARN] no prompt after passwd hash"
            }
            Send-Cmd -Cmd "passwd --status"
        } else {
            Write-Host "`n[OK] root already provisioned - skipping passwd"
        }

        $beforeLogin = Get-PromptCount
        $markLogin = $script:sb.Length
        Send-Line -Cmd "login"
        $sawLogin = Wait-ForNewText -Needle "root password:" -AfterLength $markLogin -TimeoutSec 15
        if (-not $sawLogin) {
            Write-Host "`n[WARN] did not see login prompt - sending password anyway"
        }
        Write-Host "`n>>> sending login secret"
        $sp.Write("$Password`r")
        if (-not (Wait-ForPrompt -Target ($beforeLogin + 1) -TimeoutSec $HashTimeoutSec)) {
            Write-Host "`n[WARN] no prompt after login hash"
        }

        Send-Cmd -Cmd "mod -l"
        Send-Cmd -Cmd "mod shell"
        Send-Cmd -Cmd "mod fs.rm"
        Send-Cmd -Cmd "logout"
        Send-Cmd -Cmd "mod fs.rm"

        $beforeBad = Get-PromptCount
        $markBad = $script:sb.Length
        Send-Line -Cmd "login"
        $sawBad = Wait-ForNewText -Needle "root password:" -AfterLength $markBad -TimeoutSec 15
        if (-not $sawBad) {
            Write-Host "`n[WARN] did not see failed-login prompt - sending password anyway"
        }
        Write-Host "`n>>> sending wrong-password secret"
        $sp.Write("this is a long but wrong password xx`r")
        if (-not (Wait-ForPrompt -Target ($beforeBad + 1) -TimeoutSec $HashTimeoutSec)) {
            Write-Host "`n[WARN] no prompt after failed login hash"
        }

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
