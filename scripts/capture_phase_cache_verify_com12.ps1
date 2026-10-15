# Live COM12 verification for the real two-clock PhaseCache
# (kernel::phase_cache) on PZ7020-SL-C (kairos_zynq).
#
# Flow: open COM12 (115200 8N1) -> you press RESET -> capture the boot banner
# until "gpos> " -> drive write/stat/cat/phasecache/checkpoint through the
# real serial console to produce, in order: a real cache miss (populate), a
# real cache hit shared across stat+cat, a real checkpoint seal (genesis
# entries stay trusted across it), a real invalidating write, a real
# trust-rejection re-fetch (miss), and a second real checkpoint seal.
#
# Drives the REAL interactive console over PS UART0 RX/TX. Every response is
# the board's; nothing here is faked, replayed, or scripted kernel-side.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts\capture_phase_cache_verify_com12.ps1
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
$capFile = Join-Path $OutDir "com12_phase_cache_verify_$stamp.txt"

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
        Write-Host "`n[OK] reached 'gpos> ' - kernel shell is up. Driving PhaseCache-backed fs commands..."
        [void](Send-Cmd -Cmd "mkdir /cache")
        [void](Send-Cmd -Cmd "write /cache/a.txt hello-phasecache")

        Write-Host "`n=== baseline (expect entries=0 hits=0 misses=0 trust_rejections=0) ==="
        [void](Send-Cmd -Cmd "phasecache")

        Write-Host "`n=== first stat: real cache miss, populates the entry ==="
        [void](Send-Cmd -Cmd "stat /cache/a.txt")
        [void](Send-Cmd -Cmd "phasecache")

        Write-Host "`n=== cat of the same path: real cache HIT (shared with the stat above) ==="
        [void](Send-Cmd -Cmd "cat /cache/a.txt")
        [void](Send-Cmd -Cmd "phasecache")

        Write-Host "`n=== repeat stat: another real HIT ==="
        [void](Send-Cmd -Cmd "stat /cache/a.txt")
        [void](Send-Cmd -Cmd "phasecache")

        Write-Host "`n=== first real checkpoint seal (sequence 0) ==="
        [void](Send-Cmd -Cmd "checkpoint")

        Write-Host "`n=== stat after the checkpoint: genesis-bound entry stays trusted -> still a HIT ==="
        [void](Send-Cmd -Cmd "stat /cache/a.txt")
        [void](Send-Cmd -Cmd "phasecache")

        Write-Host "`n=== invalidating write to the same path ==="
        [void](Send-Cmd -Cmd "write /cache/a.txt updated-content")

        Write-Host "`n=== stat after the write: real trust-rejection MISS, then re-fetch/repopulate ==="
        [void](Send-Cmd -Cmd "stat /cache/a.txt")
        [void](Send-Cmd -Cmd "phasecache")

        Write-Host "`n=== cat after the re-fetch: real HIT again, and content reflects the write ==="
        [void](Send-Cmd -Cmd "cat /cache/a.txt")
        [void](Send-Cmd -Cmd "phasecache")

        Write-Host "`n=== second real checkpoint seal (sequence 1) ==="
        [void](Send-Cmd -Cmd "checkpoint")
        [void](Send-Cmd -Cmd "phasecache")

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
