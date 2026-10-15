# iCE Max Wave A regression — reflash fixed matrix; host-verify each gate.
param(
    [switch]$SkipGates,
    [switch]$SkipAlgebra,
    [switch]$SkipGlider
)
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot
$CAD = "C:\Tools\oss-cad-suite\oss-cad-suite"
$env:PATH = "$CAD\bin;$CAD\lib;$env:PATH"

$rollup = Join-Path $PSScriptRoot ("evidence\ICE_MAX_CAMPAIGN_{0:yyyyMMdd_HHmmss}.md" -f (Get-Date))
$log = Join-Path $PSScriptRoot "evidence\ice_regression_log.txt"
"ICE regression $(Get-Date -Format o)" | Set-Content $log
@(
    "# iCE Max regression"
    ""
    "Board: iCEBreaker V1.1a · COM24 · 12 MHz"
    "Started: $(Get-Date -Format o)"
    ""
    "| Gate | Result | SHA-256 |"
    "|------|--------|---------|"
) | Set-Content $rollup

function Add-Row($gate, $result, $sha) {
    "| $gate | $result | ``$sha`` |" | Add-Content $rollup
    "$result $gate $sha" | Add-Content $log
}

function Run-Script($name, $script, $shaHint) {
    Write-Host "==== REGRESS $name ===="
    & $script
    if ($LASTEXITCODE -ne 0 -and -not $?) { throw "failed $name" }
    $sha = "unknown"
    if (Test-Path $shaHint) {
        $sha = ((Get-Content $shaHint -Raw) -split '\s+')[0]
    }
    Add-Row $name "PASS" $sha
}

if (-not $SkipGates) {
    Run-Script "complex_gates" { .\flash-capture-complex-gates.ps1 } "evidence\complex_gates\icebreaker.bin.sha256"
    Run-Script "reflect_topology" { .\flash-capture-reflect-topology.ps1 } "evidence\reflect_topology\icebreaker.bin.sha256"
    Run-Script "gpl_vm" { .\flash-capture-gpl-vm.ps1 } "evidence\gpl_vm\icebreaker.bin.sha256"
}

if (-not $SkipAlgebra) {
    Run-Script "gpl_algebra_64" { .\flash-capture-gpl-algebra-64.ps1 } "evidence\gpl_algebra_64\icebreaker.bin.sha256"
}

if (-not $SkipGlider) {
    Run-Script "glider_V1D2_S22" {
        .\flash-capture-phase-lattice-2d-glider.ps1 -M 4 -GENS 8 -VACUUM 1 -DEFECT 2 -SEED_R 2 -SEED_C 2
    } "evidence\phase_lattice_2d_glider_M4_G8_V1D2_S22\icebreaker.bin.sha256"
    Run-Script "glider_V2D1_S22" {
        .\flash-capture-phase-lattice-2d-glider.ps1 -M 4 -GENS 8 -VACUUM 2 -DEFECT 1 -SEED_R 2 -SEED_C 2
    } "evidence\phase_lattice_2d_glider_M4_G8_V2D1_S22\icebreaker.bin.sha256"
}

"" | Add-Content $rollup
"Finished: $(Get-Date -Format o)" | Add-Content $rollup
"REGRESSION COMPLETE" | Add-Content $log
Write-Host "Regression complete — $rollup"
