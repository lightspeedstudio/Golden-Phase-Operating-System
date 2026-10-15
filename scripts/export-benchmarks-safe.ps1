# Safe benchmark export: clean existing raw output without re-running fib(1M) unless forced.
# Usage:
#   .\scripts\export-benchmarks-safe.ps1              # clean from .raw.txt only
#   .\scripts\export-benchmarks-safe.ps1 -ForceReRun  # re-run heavy tests (slow!)
param(
    [switch]$ForceReRun
)

$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root

$env:CARGO_BUILD_JOBS = "1"
$env:CARGO_INCREMENTAL = "0"

$snapDir = Join-Path $root "release_snapshot\benchmarks"
New-Item -ItemType Directory -Force -Path $snapDir | Out-Null

function Remove-CargoHarnessNoise {
    param([string[]]$Lines)
    $Lines | Where-Object {
        $_ -notmatch '^\s*running \d+ tests' -and
        $_ -notmatch '^\s*test result:' -and
        $_ -notmatch '^\s*test \S+ has been running' -and
        $_ -notmatch '^\s*test \S+ \.\.\. (ok|FAILED)' -and
        $_ -notmatch '^\s*Doc-tests' -and
        $_ -notmatch '^\s*Compiling ' -and
        $_ -notmatch '^\s*Finished ' -and
        $_ -notmatch '^\s*Running ' -and
        $_ -notmatch '^\s*warning:' -and
        $_ -notmatch '^\s*error:' -and
        $_ -notmatch '^\s*note:' -and
        $_ -notmatch '^\s*help:' -and
        $_ -notmatch '^\s*At .+ps1:\d+ char:\d+' -and
        $_ -notmatch '^\s*\+ ' -and
        $_ -notmatch '^\s*CategoryInfo' -and
        $_ -notmatch '^\s*FullyQualifiedErrorId'
    }
}

function Get-JsonBlock {
    param([string]$Text)
    $start = $Text.IndexOf('{')
    if ($start -lt 0) { return $null }
    $depth = 0
    $inString = $false
    $escape = $false
    for ($i = $start; $i -lt $Text.Length; $i++) {
        $c = $Text[$i]
        if ($inString) {
            if ($escape) { $escape = $false; continue }
            if ($c -eq '\') { $escape = $true; continue }
            if ($c -eq '"') { $inString = $false }
            continue
        }
        switch ($c) {
            '"' { $inString = $true }
            '{' { $depth++ }
            '}' {
                $depth--
                if ($depth -eq 0) {
                    return $Text.Substring($start, $i - $start + 1)
                }
            }
        }
    }
    return $null
}

function Export-CleanJsonFromRaw {
    param(
        [string]$RawPath,
        [string]$OutPath,
        [string]$Label
    )

    if (-not (Test-Path $RawPath)) {
        Write-Warning "[$Label] missing raw file: $RawPath"
        return $false
    }

    $raw = Get-Content -Raw -Encoding UTF8 $RawPath
    $cleaned = (Remove-CargoHarnessNoise -Lines ($raw -split "`r?`n")) -join "`n"
    $json = Get-JsonBlock -Text $cleaned
    if (-not $json) {
        Write-Warning "[$Label] no JSON object found in $RawPath"
        return $false
    }

    try {
        $null = $json | ConvertFrom-Json
    } catch {
        Write-Warning "[$Label] JSON validation failed: $_"
        return $false
    }

    [IO.File]::WriteAllText($OutPath, $json.Trim() + "`n", [Text.UTF8Encoding]::new($false))
    Write-Host "[$Label] wrote $($OutPath) ($($json.Length) bytes)"
    return $true
}

function Invoke-HeavyBenchmark {
    param(
        [string]$TestFilter,
        [string]$RawPath
    )

    Write-Host "Re-running heavy benchmark: $TestFilter (CARGO_BUILD_JOBS=1)..."
    Write-Warning "fib(1_000_000) can take 10+ minutes at 100% CPU. Run alone."
    $stderr = Join-Path $env:TEMP "kairos_benchmark_err.txt"
    & cargo test --jobs 1 $TestFilter -- --nocapture 1> $RawPath 2> $stderr
    if ($LASTEXITCODE -ne 0) {
        $err = if (Test-Path $stderr) { Get-Content $stderr -Raw } else { "" }
        throw "cargo test $TestFilter failed (exit $LASTEXITCODE): $err"
    }
}

$exports = @(
    @{
        Label      = "phase_graph"
        Raw        = Join-Path $snapDir "phase_graph_data.json.raw.txt"
        Out        = Join-Path $snapDir "phase_graph_data.json"
        TestFilter = "--test phase_benchmark_graph_data export_graph_data"
    },
    @{
        Label      = "phase_scale"
        Raw        = Join-Path $snapDir "phase_scale_data.json.raw.txt"
        Out        = Join-Path $snapDir "phase_scale_data.json"
        TestFilter = "--test phase_benchmark_scale_data export_scale_compare_graph_data"
    }
)

$ok = 0
foreach ($item in $exports) {
    $success = Export-CleanJsonFromRaw -RawPath $item.Raw -OutPath $item.Out -Label $item.Label
    if (-not $success) {
        if ($ForceReRun) {
            Invoke-HeavyBenchmark -TestFilter $item.TestFilter -RawPath $item.Raw
            $success = Export-CleanJsonFromRaw -RawPath $item.Raw -OutPath $item.Out -Label $item.Label
        } else {
            Write-Warning "[$($item.Label)] clean export failed; pass -ForceReRun to regenerate (slow)."
        }
    }
    if ($success) { $ok++ }
}

Write-Host "Done: $ok / $($exports.Count) benchmark JSON files ready."
if ($ok -lt $exports.Count) { exit 1 }
