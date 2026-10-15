# COM12 capture for mmu-app-isolation. DTR/RTS off. Press RESET after SD copy.
param(
    [string]$Port = "COM12",
    [int]$Baud = 115200,
    [int]$BootTimeoutSec = 90,
    [int]$CmdTimeoutSec = 60,
    [string]$OutDir = "$PSScriptRoot\..\fpga\zynq\evidence\kairos_zynq_boot\BOOT_MMU_ISOLATION_20260815"
)

$ErrorActionPreference = "Stop"
$prompt = "gpos> "
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$stamp = Get-Date -Format "yyyyMMdd_HHmmss"
$capFile = Join-Path $OutDir "com12_mmuiso_$stamp.txt"

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

function Wait-ForText {
    param([string]$Needle, [int]$TimeoutSec)
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while ((Get-Date) -lt $deadline) {
        Read-Available
        if ($script:sb.ToString().Contains($Needle)) { return $true }
        Start-Sleep -Milliseconds 40
    }
    return $false
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
    param([string]$Cmd, [int]$TimeoutSec)
    $before = Get-PromptCount
    $sp.Write($Cmd + "`r")
    if (-not (Wait-ForPrompt ($before + 1) $TimeoutSec)) {
        Write-Host "`nTIMEOUT after: $Cmd"
        return $false
    }
    return $true
}

try {
    $sp.Open()
} catch {
    Write-Host "Cannot open $Port : $($_.Exception.Message)"
    exit 1
}

$dbgLog = "C:\Users\richa\OneDrive\Desktop\FullOS\debug-638b5a.log"
function Write-Dbg([string]$HypothesisId, [string]$Message, [hashtable]$Data) {
    $obj = [ordered]@{
        sessionId = "638b5a"
        runId = "mmuiso-matrix-1"
        hypothesisId = $HypothesisId
        location = "capture_mmuiso_com12.ps1"
        message = $Message
        data = $Data
        timestamp = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
    }
    ($obj | ConvertTo-Json -Compress -Depth 6) | Add-Content -Path $dbgLog -Encoding UTF8
}

Write-Host "=== $Port open DTR/RTS off. If no prompt, PRESS PZ7020 RESET ==="
try {
    $sp.Write("`r")
    $already = Wait-ForPrompt 1 3
    if (-not $already) {
        Write-Host "No prompt yet. PRESS PZ7020 RESET now."
        $gotIso = Wait-ForText "MMU: CPU0 isolation" $BootTimeoutSec
        if (-not $gotIso) {
            Write-Host "`nNO isolation banner in ${BootTimeoutSec}s (reset / SD?)"
            Write-Dbg "A" "no_banner" @{ len = $script:sb.Length }
            [System.IO.File]::WriteAllText($capFile, $script:sb.ToString())
            Write-Host "wrote $capFile"
            exit 2
        }
        if (-not (Wait-ForPrompt 1 30)) {
            Write-Host "`nNO gpos> after isolation banner"
            Write-Dbg "A" "no_prompt" @{ len = $script:sb.Length }
            [System.IO.File]::WriteAllText($capFile, $script:sb.ToString())
            exit 3
        }
    }
    $ok = $true
    $ok = (Send-Cmd "mmuiso status" $CmdTimeoutSec) -and $ok
    $ok = (Send-Cmd "mmuiso selftest" 90) -and $ok
    $ok = (Send-Cmd "version" 15) -and $ok
    Read-Available
    $raw = $script:sb.ToString()
    [System.IO.File]::WriteAllText($capFile, $raw)
    Write-Host "`nwrote $capFile"
    if (-not $ok) {
        Write-Host "TIMEOUT: gpos> is transport only; not a matrix PASS"
        exit 4
    }
    # A03: ordered single-run classifier. Prompt-return cannot PASS.
    $a03 = Join-Path $PSScriptRoot "a03_mmuiso_matrix.py"
    & python $a03 $capFile
    $code = $LASTEXITCODE
    Write-Dbg "M" "a03" @{
        ok_prompt = $ok
        a03_exit = $code
        cap = $capFile
        len = $raw.Length
    }
    exit $code
}
finally {
    $sp.Close()
}
