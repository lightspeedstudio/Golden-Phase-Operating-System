# Live COM12 proof for PhaseStack signed GPA launch v1.
#
# Pre-req: SD FAT root has HELLO.GPA (signed), BAD.GPA (one-byte flip),
# UNK.GPA (unknown signer), OLD.GPA (retired research CA). BOOT is gold
# PATH1_G64 bit + Utility ELF built without phasestack-sign-dev.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts\capture_phasestack_signed_gpa_com12.ps1

param(
    [string]$Port = "COM12",
    [int]$Baud = 115200,
    [int]$BootTimeoutSec = 90,
    [int]$CmdTimeoutSec = 30,
    [string]$OutDir = "$PSScriptRoot\..\fpga\zynq\evidence\kairos_zynq_boot"
)

$ErrorActionPreference = "Stop"
$prompt = "gpos> "

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$stamp = Get-Date -Format "yyyyMMdd_HHmmss"
$capFile = Join-Path $OutDir "com12_phasestack_signed_gpa_$stamp.txt"

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
    Send-Cmd "app list"
    Send-Cmd "fs ls"
    Send-Cmd "app verify hello.gpa"
    $installOut = Send-Cmd "app install hello.gpa"
    Send-Cmd "app list"
    $id = $null
    if ($installOut -match '([0-9a-f]{64})') {
        # list prints app id first
    }
    $listOut = Send-Cmd "app list"
    if ($listOut -match '(?m)^([0-9a-f]{64})\s+hello-a7f3c91e') {
        $id = $Matches[1]
    }
    if (-not $id) {
        Send-Cmd "app info missing"
    } else {
        Send-Cmd "app info $id"
        Send-Cmd "app run $id"
        Send-Cmd "app stop $id"
        Send-Cmd "app revoke $id"
        Send-Cmd "app run $id"
    }
    Send-Cmd "app install bad.gpa"
    Send-Cmd "app install unk.gpa"
    Send-Cmd "app install old.gpa"
    Send-Cmd "checkpoint"
}
finally {
    $text = $script:sb.ToString()
    [System.IO.File]::WriteAllText($capFile, $text)
    if ($sp.IsOpen) { $sp.Close() }
    Write-Host "`nWrote $capFile"
}
