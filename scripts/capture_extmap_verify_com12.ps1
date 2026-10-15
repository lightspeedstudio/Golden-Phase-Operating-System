# Live COM12 verification for the real storage allocation and extent-map
# subsystem (kernel::extmap) on PZ7020-SL-C (kairos_zynq), Subsystem 6
# (extmap) Gate E.
#
# Flow: open COM12 (115200 8N1) -> you press RESET -> capture the boot banner
# until "gpos> " -> baseline `extmap` (nothing allocated yet) -> write two
# distinct single-block files with IDENTICAL content (real dedup_hits, real
# shared block, refcount=2) -> cross-check `extmap` -> `cat` both back ->
# write a genuine multi-block file (1300 bytes -> 3 real blocks) -> `cat` it
# back exactly -> cross-check `extmap` (blocks_live +3) -> overwrite that
# same path with short content (real "free 3 old blocks, claim 1 new one"
# accounting) -> `cat` the shrunk content back -> `rm` it (real delete) ->
# `rm` one of the two dedup-sharing files (refcount 2->1, block must stay
# Live for the other file) -> `cat` the other dedup file (still readable --
# proves the shared block was not wrongly freed) -> `rm` it too (refcount
# 1->0, block genuinely freed) -> final `extmap` (back to the exact
# baseline).
#
# Drives the REAL interactive console over PS UART0 RX/TX. Every response is
# the board's; nothing here is faked, replayed, or scripted kernel-side.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts\capture_extmap_verify_com12.ps1
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
$capFile = Join-Path $OutDir "com12_extmap_verify_$stamp.txt"

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

$dupContent = "extmap-dedup-shared-content"
$multiContent = "0123456789" * 130   # 1300 bytes -> ceil(1300/512) = 3 blocks
$shrunkContent = "shrunk-down-content"

try {
    $sp.Open()
    Write-Host "LISTENING $Port @ $Baud - press RESET on the board now (up to $BootTimeoutSec sec for 'gpos> ')..."

    $booted = Wait-ForPrompt -Target 1 -TimeoutSec $BootTimeoutSec
    if (-not $booted) {
        Write-Host "`n[FAIL] no 'gpos> ' prompt within $BootTimeoutSec sec - board did not reach the shell."
    }
    else {
        Write-Host "`n[OK] reached 'gpos> ' - kernel shell is up. Driving extmap-backed commands..."

        Write-Host "`n=== baseline extmap stats (nothing allocated yet) ==="
        [void](Send-Cmd -Cmd "extmap")

        Write-Host "`n=== write file A with content that will be dedup-eligible (single block) ==="
        [void](Send-Cmd -Cmd "write /extmap_dup_a.txt $dupContent")

        Write-Host "`n=== extmap after A: blocks_live +1, writes +1, dedup_hits still 0 ==="
        [void](Send-Cmd -Cmd "extmap")

        Write-Host "`n=== write file B with IDENTICAL content: must be a real dedup hit, sharing A's block ==="
        [void](Send-Cmd -Cmd "write /extmap_dup_b.txt $dupContent")

        Write-Host "`n=== extmap after B: dedup_hits +1, but blocks_live UNCHANGED (one block backs both files) ==="
        [void](Send-Cmd -Cmd "extmap")

        Write-Host "`n=== cat both dedup files back: both must return the exact same content ==="
        [void](Send-Cmd -Cmd "cat /extmap_dup_a.txt")
        [void](Send-Cmd -Cmd "cat /extmap_dup_b.txt")

        Write-Host "`n=== write a genuine multi-block file (1300 bytes -> 3 real blocks) ==="
        [void](Send-Cmd -Cmd "write /extmap_multi.txt $multiContent")

        Write-Host "`n=== extmap after multi-block write: blocks_live +3 ==="
        [void](Send-Cmd -Cmd "extmap")

        Write-Host "`n=== cat the multi-block file back: must return the exact 1300-byte content ==="
        [void](Send-Cmd -Cmd "cat /extmap_multi.txt")

        Write-Host "`n=== overwrite the SAME path with short content: must free the 3 old blocks and claim exactly 1 new one ==="
        [void](Send-Cmd -Cmd "write /extmap_multi.txt $shrunkContent")

        Write-Host "`n=== extmap after overwrite: blocks_live must drop back down by 2 net (3 freed, 1 claimed) ==="
        [void](Send-Cmd -Cmd "extmap")

        Write-Host "`n=== cat the now-shrunk file: must return the NEW short content, not the old multi-block one ==="
        [void](Send-Cmd -Cmd "cat /extmap_multi.txt")

        Write-Host "`n=== rm the shrunk file: its one remaining block must be freed ==="
        [void](Send-Cmd -Cmd "rm /extmap_multi.txt")

        Write-Host "`n=== extmap after rm: only the dedup-shared block should still be live ==="
        [void](Send-Cmd -Cmd "extmap")

        Write-Host "`n=== rm dedup file A: refcount 2->1, the SHARED block must stay Live (B still references it) ==="
        [void](Send-Cmd -Cmd "rm /extmap_dup_a.txt")

        Write-Host "`n=== extmap after rm A: blocks_live must be UNCHANGED (still 1) -- the shared block was not freed ==="
        [void](Send-Cmd -Cmd "extmap")

        Write-Host "`n=== cat dedup file B: must STILL work -- proves the shared block was not wrongly freed by rm A ==="
        [void](Send-Cmd -Cmd "cat /extmap_dup_b.txt")

        Write-Host "`n=== rm dedup file B: refcount 1->0, the shared block must NOW be genuinely freed ==="
        [void](Send-Cmd -Cmd "rm /extmap_dup_b.txt")

        Write-Host "`n=== final extmap stats: must be back to the EXACT baseline (blocks_free=1024, blocks_live=0) ==="
        [void](Send-Cmd -Cmd "extmap")

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
