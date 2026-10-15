# PZ7020 physical test harness — drives the REAL Golden Phase OS console over
# PS UART0 / COM12. One command at a time, through real UART RX. Never fabricates
# input inside the kernel; every command travels over the wire.
#
# Safety:
#   - Runs only commands from a plain-text plan file (one per line, # = comment).
#   - A built-in DESTRUCTIVE denylist (kill, rm, exit, anom off/rekey/unprotect,
#     ssh stop/restart, clear) is refused UNLESS -AllowDestructive is passed AND
#     the command is also present in the plan. Nothing destructive is ever
#     auto-added.
#
# Outputs (under fpga/zynq/evidence/kairos_zynq_boot/<run-id>/):
#   transcript.txt   raw bytes captured (full session)
#   results.json     machine-readable per-command results
#   report.md        human-readable report
#   SHA256SUMS.txt   sha256 of transcript.txt (and this run's artifacts)
#
# Anomaly detection per command: panic / abort / reset / hang / lost-prompt.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts\pz7020_test_harness.ps1 `
#       -Plan scripts\pz7020_test_plan_default.txt
#   ... -Port COM12 -CmdTimeoutSec 15 -AllowDestructive:$false

param(
    [string]$Plan,
    [string]$Port = "COM12",
    [int]$Baud = 115200,
    [int]$BootTimeoutSec = 60,
    [int]$CmdTimeoutSec = 15,
    [switch]$AllowDestructive = $false,
    [string]$OutRoot = "$PSScriptRoot\..\fpga\zynq\evidence\kairos_zynq_boot"
)

$ErrorActionPreference = "Stop"
$prompt = "gpos> "

# Commands that change state or end the session. Never auto-run.
# Note: `write /tmp/...` is allowed (PhaseFS RAM verification). Other writes need -AllowDestructive.
$destructive = @('kill','rm','exit','clear','anom off','anom rekey','anom unprotect',
                 'ssh stop','ssh restart','mkdir','unlock','lock')

# Patterns that mark a failure/instability in captured output.
$panicPat = 'panic|PANIC'
$abortPat = '(^|\r|\n)D![0-9A-Fa-f]|(^|\r|\n)U(\r|\n)|(^|\r|\n)P(\r|\n)|abort'
$resetPat = 'Golden Phase Operating System boot splash|PS7 INIT|BootROM'

if (-not $Plan) {
    $Plan = Join-Path $PSScriptRoot "pz7020_test_plan_feature.txt"
}
if (-not (Test-Path $Plan)) { throw "Plan file not found: $Plan" }

$runId = "run_" + (Get-Date -Format "yyyyMMdd_HHmmss")
$outDir = Join-Path $OutRoot $runId
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
$transcriptFile = Join-Path $outDir "transcript.txt"
$jsonFile = Join-Path $outDir "results.json"
$mdFile = Join-Path $outDir "report.md"
$shaFile = Join-Path $outDir "SHA256SUMS.txt"
$debugLog = Join-Path $outDir "harness_debug.ndjson"

function Write-DebugNdjson {
    param([string]$HypothesisId, [string]$Location, [string]$Message, [hashtable]$Data)
    $obj = [ordered]@{
        ts = (Get-Date).ToString("o")
        hypothesisId = $HypothesisId
        location = $Location
        message = $Message
        data = $Data
    }
    ($obj | ConvertTo-Json -Compress -Depth 6) | Add-Content -Path $debugLog -Encoding UTF8
}

function Test-IsDestructive {
    param([string]$cmd)
    if ($cmd -match '^write\s+/tmp(/|\s|$)') { return $false }
    if ($cmd -match '^write(\s|$)') { return $true }
    foreach ($d in $destructive) {
        if ($cmd -eq $d -or $cmd.StartsWith("$d ")) { return $true }
    }
    return $false
}

# Parse plan.
$commands = @()
foreach ($raw in Get-Content $Plan) {
    $line = $raw.Trim()
    if ($line -eq "" -or $line.StartsWith("#")) { continue }
    $commands += $line
}

$sp = New-Object System.IO.Ports.SerialPort $Port, $Baud, ([System.IO.Ports.Parity]::None), 8, ([System.IO.Ports.StopBits]::One)
$sp.ReadTimeout = 250
$sp.DtrEnable = $true
$sp.RtsEnable = $true

$script:sb = New-Object System.Text.StringBuilder

function Read-Available {
    try {
        $chunk = $sp.ReadExisting()
        if ($chunk) { [void]$script:sb.Append($chunk); Write-Host -NoNewline $chunk }
    } catch {}
}
function Get-PromptCount { ([regex]::Matches($script:sb.ToString(), [regex]::Escape($prompt))).Count }
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

$results = @()
$startedIso = (Get-Date).ToString("o")

try {
    $sp.Open()
    Write-Host "HARNESS $Port @ $Baud - press RESET now (up to $BootTimeoutSec sec for boot)..."
    $bootStart = Get-Date
    $booted = Wait-ForPrompt -Target 1 -TimeoutSec $BootTimeoutSec
    $bootText = $script:sb.ToString()
    $bootOk = $booted -and ($bootText -notmatch $panicPat)
    $results += [pscustomobject]@{
        command = "(boot)"; sent = $false
        started = $bootStart.ToString("o"); ended = (Get-Date).ToString("o")
        prompt_returned = $booted
        panic = [bool]($bootText -match $panicPat)
        abort = [bool]($bootText -match $abortPat)
        reset = $false
        classification = if ($bootOk) { "PHYSICALLY VERIFIED" } else { "FAILED" }
        note = if ($booted) { "reached gpos>" } else { "no prompt within boot timeout" }
    }
    #region agent log
    Write-DebugNdjson -HypothesisId "B" -Location "pz7020_test_harness.ps1:boot" -Message "boot prompt wait" -Data @{
        booted = $booted; boot_ok = $bootOk; g1p_line = [bool]($bootText -match 'G1P:'); 
        arch_splash = if ($bootText -match 'baremetal-[^\s\)]+') { $Matches[0] } else { $null }
        has_riscv_label = [bool]($bootText -match 'baremetal-riscv64')
        has_arm_label = [bool]($bootText -match 'baremetal-armv7')
    }
    #endregion
    if (-not $booted) { throw "Board did not reach gpos> within $BootTimeoutSec sec." }

    foreach ($cmd in $commands) {
        $isDestructive = Test-IsDestructive $cmd
        if ($isDestructive -and -not $AllowDestructive) {
            Write-Host "`n[SKIP destructive] $cmd"
            $results += [pscustomobject]@{
                command = $cmd; sent = $false; started = $null; ended = $null
                prompt_returned = $null; panic = $false; abort = $false; reset = $false
                classification = "SKIPPED-DESTRUCTIVE"; note = "denylisted; -AllowDestructive not set"
            }
            continue
        }

        # Longer timeout for multi-line selftests.
        $timeout = $CmdTimeoutSec
        if ($cmd -match '^(g1p selftest|g1p irq-test|phi-stability run)') { $timeout = [Math]::Max($CmdTimeoutSec, 60) }

        $before = Get-PromptCount
        $preLen = $script:sb.Length
        $startIso = (Get-Date).ToString("o")
        Write-Host "`n>>> $cmd"
        $sp.Write("$cmd`r")
        $ok = Wait-ForPrompt -Target ($before + 1) -TimeoutSec $timeout
        $endIso = (Get-Date).ToString("o")

        $reply = $script:sb.ToString().Substring($preLen)
        $panic = [bool]($reply -match $panicPat)
        $abort = [bool]($reply -match $abortPat)
        $reset = [bool]($reply -match $resetPat)

        # Distinguish "command reached handler over UART" from "feature works".
        $hostedOnly = [bool]($reply -match 'hosted-only|hosted std build required|hosted \(std\) build|hosted SSH build required|hosted build required|requires hosted|unavailable \(hosted|requires bm-net')
        $stubMsg = [bool]($reply -match '(^|[\r\n])\s*(stub|STUB)\b|MUST NOT CLAIM|requires hosted crypto')
        $unsupported = [bool]($reply -match '(^|[\r\n])[^\r\n]*\bUNSUPPORTED\b') -and -not [bool]($reply -match 'MAC loopback: PASS|register presence:.*OK|PHY ID:.*OK')
        # Prefer ladder PASS/OK over a trailing "6 external ... UNSUPPORTED" line.
        if ($reply -match 'MAC loopback: PASS' -or $reply -match 'gem (probe|phy|loopback):') {
            if ($reply -match 'gem loopback: PASS|gem phy: addr=|gem probe: NWCTRL=' -or $reply -match '1 register presence:.*OK') {
                $unsupported = $false
            }
        }
        $unknownCmd = [bool]($reply -match 'unknown command:')
        $failToken = [bool]($reply -match '(^|[\r\n]).*\bFAIL\b|MISMATCH')
        $class =
            if ($panic -or $abort) { "FAILED" }
            elseif ($reset) { "FAILED" }         # unexpected reboot mid-command
            elseif (-not $ok) { "FAILED" }        # hang / lost prompt
            elseif ($unknownCmd) { "FAILED" }     # command not present in this image
            elseif ($failToken) { "FAILED" }
            elseif ($hostedOnly) { "HOSTED ONLY" }
            elseif ($stubMsg) { "STUB - MUST NOT CLAIM PASS" }
            elseif ($unsupported) { "UNSUPPORTED" }
            else { "PHYSICALLY VERIFIED" }        # live non-stub reply + prompt

        $note =
            if ($panic) { "panic in reply" }
            elseif ($abort) { "CPU abort letter in reply" }
            elseif ($reset) { "board reset mid-command" }
            elseif (-not $ok) { "no prompt within $timeout sec (hang/lost prompt)" }
            elseif ($unknownCmd) { "unknown command - wrong/old BOOT image?" }
            elseif ($failToken) { "command reported FAIL/MISMATCH" }
            elseif ($hostedOnly) { "honest hosted-only / gate fallback over UART" }
            elseif ($stubMsg) { "stub response over UART" }
            elseif ($unsupported) { "honest unsupported over UART" }
            else { "prompt returned" }

        $results += [pscustomobject]@{
            command = $cmd; sent = $true; started = $startIso; ended = $endIso
            prompt_returned = $ok; panic = $panic; abort = $abort; reset = $reset
            classification = $class; note = $note
        }

        #region agent log
        $hyp = "B"
        if ($cmd -eq "version" -or $cmd -eq "host") { $hyp = "A" }
        elseif ($cmd -in @("phasenet","phaselink","ssh","ai","phaseperf","phase-benchmark","checkpoint","audit","root")) { $hyp = "C" }
        elseif ($cmd -eq "state" -or $cmd -eq "clock") { $hyp = "D" }
        elseif ($cmd -eq "net") { $hyp = "E" }
        $snippet = if ($reply.Length -gt 400) { $reply.Substring(0,400) } else { $reply }
        Write-DebugNdjson -HypothesisId $hyp -Location "pz7020_test_harness.ps1:cmd" -Message "command result" -Data @{
            command = $cmd; classification = $class; prompt_returned = $ok
            panic = $panic; abort = $abort; reset = $reset
            has_riscv_label = [bool]($reply -match 'baremetal-riscv64')
            has_arm_label = [bool]($reply -match 'baremetal-armv7|Cortex-A9|armv7')
            hosted_msg = [bool]($reply -match 'hosted|HOSTED|std build required|stub')
            reply_snippet = ($snippet -replace "[\r\n]+"," | ")
        }
        #endregion

        if ($panic -or $abort -or $reset) {
            Write-Host "`n[STOP] instability after '$cmd' ($note). Halting to preserve evidence."
            break
        }
    }
}
finally {
    if ($sp.IsOpen) { $sp.Close() }
    $sp.Dispose()
}

# Persist transcript.
$script:sb.ToString() | Set-Content $transcriptFile -Encoding UTF8
$transcriptSha = (Get-FileHash $transcriptFile -Algorithm SHA256).Hash.ToLower()

# JSON.
$summary = [pscustomobject]@{
    run_id = $runId
    port = $Port; baud = $Baud
    started = $startedIso; ended = (Get-Date).ToString("o")
    plan = (Resolve-Path $Plan).Path
    transcript = "transcript.txt"
    transcript_sha256 = $transcriptSha
    commands = $results
}
$summary | ConvertTo-Json -Depth 6 | Set-Content $jsonFile -Encoding UTF8

# Markdown.
$md = @()
$md += "# PZ7020 Physical Test Run - $runId"
$md += ""
$md += "- Port: $Port @ $Baud 8-N-1"
$md += "- Plan: $((Resolve-Path $Plan).Path)"
$md += "- Started: $startedIso"
$md += "- Transcript: ``transcript.txt`` sha256 ``$transcriptSha``"
$md += ""
$md += "| # | command | sent | prompt | panic | abort | reset | classification | note |"
$md += "|---|---------|------|--------|-------|-------|-------|----------------|------|"
$i = 0
foreach ($r in $results) {
    $i++
    $md += "| $i | ``$($r.command)`` | $($r.sent) | $($r.prompt_returned) | $($r.panic) | $($r.abort) | $($r.reset) | $($r.classification) | $($r.note) |"
}
$md += ""
$md += "> Only rows with classification PHYSICALLY VERIFIED and a returned prompt were confirmed on hardware. FAILED = panic/abort/reset/hang/lost-prompt. SKIPPED-DESTRUCTIVE = denylisted and not run."
$md -join "`n" | Set-Content $mdFile -Encoding UTF8

# SHA sums for all run artifacts.
$sha = @()
foreach ($f in @($transcriptFile, $jsonFile, $mdFile)) {
    $h = (Get-FileHash $f -Algorithm SHA256).Hash.ToLower()
    $sha += "$h  $(Split-Path $f -Leaf)"
}
$sha -join "`n" | Set-Content $shaFile -Encoding UTF8

#region agent log
Write-DebugNdjson -HypothesisId "B" -Location "pz7020_test_harness.ps1:complete" -Message "run complete" -Data @{
    out_dir = $outDir; transcript_sha256 = $transcriptSha
    command_count = $results.Count
    failed = @($results | Where-Object { $_.classification -eq "FAILED" }).Count
    verified = @($results | Where-Object { $_.classification -eq "PHYSICALLY VERIFIED" }).Count
}
#endregion

Write-Host "`n`n=== RUN COMPLETE ==="
Write-Host "dir:        $outDir"
Write-Host "transcript: $transcriptFile (sha256 $transcriptSha)"
Write-Host "json:       $jsonFile"
Write-Host "report:     $mdFile"
Write-Host "debug_log:  $debugLog"
