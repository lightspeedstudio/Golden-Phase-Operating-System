# Live COM12 proof for PhaseSentinel-gated signed onboard app run.
#
# Pre-req: SD FAT root has HELLO.GPA (nonce-emitting, pin-signed), BAD.GPA
# (one-byte flip), UNK.GPA, OLD.GPA, NOSIG.GPA, OVER.GPA. BOOT is gold
# PATH1_G64 bit + Utility ELF without phasestack-sign-dev / phasesentinel-research.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts\capture_phasesentinel_signed_run_com12.ps1

param(
    [string]$Port = "COM12",
    [int]$Baud = 115200,
    [int]$BootTimeoutSec = 90,
    [int]$CmdTimeoutSec = 30,
    [string]$OutDir = "$PSScriptRoot\..\fpga\zynq\evidence\kairos_zynq_boot\BOOT_PHASESENTINEL_SIGNED_RUN_20260815"
)

$ErrorActionPreference = "Stop"

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$stamp = Get-Date -Format "yyyyMMdd_HHmmss"
$capFile = Join-Path $OutDir "com12_phasesentinel_signed_run_$stamp.txt"

$sp = New-Object System.IO.Ports.SerialPort $Port, $Baud, ([System.IO.Ports.Parity]::None), 8, ([System.IO.Ports.StopBits]::One)
$sp.ReadTimeout = 300
$sp.NewLine = "`n"
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
    Write-Host "Opened $Port. Press RESET on PZ7020."
    if (-not (Wait-ForPrompt -Target 1 -TimeoutSec $BootTimeoutSec)) {
        throw "boot prompt not seen"
    }

    Send-Cmd "checkpoint"
    Send-Cmd "app registry"
    Send-Cmd "fs ls"
    Send-Cmd "app verify hello.gpa"
    Send-Cmd "app install hello.gpa"
    Send-Cmd "app registry"
    Send-Cmd "app status hello-a7f3c91e"
    Send-Cmd "app run hello-a7f3c91e"
    Send-Cmd "app status hello-a7f3c91e"
    Send-Cmd "app stop hello-a7f3c91e"
    Send-Cmd "app revoke hello-a7f3c91e"
    Send-Cmd "app run hello-a7f3c91e"
    Send-Cmd "app install bad.gpa"
    Send-Cmd "app install unk.gpa"
    Send-Cmd "app install old.gpa"
    Send-Cmd "app install nosig.gpa"
    Send-Cmd "app install over.gpa --grant-caps 1"
    Send-Cmd "app run over-cap-v1"
    Send-Cmd "checkpoint"
}
finally {
    $text = $script:sb.ToString()
    [System.IO.File]::WriteAllText($capFile, $text)
    if ($sp.IsOpen) { $sp.Close() }
    Write-Host "`nWrote $capFile"
}
