# Phase 1 campaign: all starts + reflected + soak.
param(
    [switch]$SkipSoak,
    [int]$SoakGens = 10000
)
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot
$log = Join-Path $PSScriptRoot "evidence\phase1_campaign_log.txt"
"Phase1 campaign $(Get-Date -Format o)" | Set-Content $log

function Run-One($gens, $vac, $def, $sr, $sc) {
    $name = "phase_lattice_2d_glider_M4_G${gens}_V${vac}D${def}_S${sr}${sc}"
    Write-Host "==== $name ===="
    & .\flash-capture-phase-lattice-2d-glider.ps1 -M 4 -GENS $gens -VACUUM $vac -DEFECT $def -SEED_R $sr -SEED_C $sc -OutName $name
    "PASS $name" | Add-Content $log
}

# 1) Regression center seed GENS=8
Run-One 8 1 2 2 2

# 2) All starting coordinates (GENS=8)
for ($r = 0; $r -lt 4; $r++) {
    for ($c = 0; $c -lt 4; $c++) {
        Run-One 8 1 2 $r $c
    }
}

# 3) Reflected vacuum/defect, all starts
for ($r = 0; $r -lt 4; $r++) {
    for ($c = 0; $c -lt 4; $c++) {
        Run-One 8 2 1 $r $c
    }
}

# 4) Long soak (≥10^4 gens); optionally stretch toward 10^6 cell updates
if (-not $SkipSoak) {
    Run-One $SoakGens 1 2 2 2
}

"CAMPAIGN COMPLETE" | Add-Content $log
Write-Host "Phase1 campaign complete - see $log"
