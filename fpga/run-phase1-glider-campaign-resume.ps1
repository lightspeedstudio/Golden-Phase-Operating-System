# Resume Phase 1 from reflected seeds + soak (V1D2 all-starts already PASS).
param(
    [int]$SoakGens = 10000,
    [switch]$SkipSoak
)
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot
$log = Join-Path $PSScriptRoot "evidence\phase1_campaign_log.txt"
"Phase1 resume $(Get-Date -Format o)" | Add-Content $log

function Run-One($gens, $vac, $def, $sr, $sc) {
    $name = "phase_lattice_2d_glider_M4_G${gens}_V${vac}D${def}_S${sr}${sc}"
    $stamp = Join-Path $PSScriptRoot "evidence\$name\PHYSICALLY_VERIFIED.txt"
    if (Test-Path $stamp) {
        Write-Host "SKIP (already verified) $name"
        "SKIP $name" | Add-Content $log
        return
    }
    Write-Host "==== $name ===="
    & .\flash-capture-phase-lattice-2d-glider.ps1 -M 4 -GENS $gens -VACUUM $vac -DEFECT $def -SEED_R $sr -SEED_C $sc -OutName $name
    "PASS $name" | Add-Content $log
}

for ($r = 0; $r -lt 4; $r++) {
    for ($c = 0; $c -lt 4; $c++) {
        Run-One 8 2 1 $r $c
    }
}

if (-not $SkipSoak) {
    Run-One $SoakGens 1 2 2 2
}

"CAMPAIGN COMPLETE" | Add-Content $log
Write-Host "Phase1 resume complete - see $log"
