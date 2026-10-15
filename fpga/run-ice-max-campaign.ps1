# iCE Max Wave A–D board runner (ordered).
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot
$log = Join-Path $PSScriptRoot "evidence\ice_max_run_log.txt"
"ICE MAX RUN $(Get-Date -Format o)" | Set-Content $log

function Step($name, $script) {
    Write-Host "==== $name ===="
    & $script
    "PASS $name" | Add-Content $log
}

# Wave A regress (core)
Step "algebra64" { .\flash-capture-gpl-algebra-64.ps1 }
Step "glider_V1D2" { .\flash-capture-phase-lattice-2d-glider.ps1 -M 4 -GENS 8 -VACUUM 1 -DEFECT 2 -SEED_R 2 -SEED_C 2 }
Step "glider_V2D1" { .\flash-capture-phase-lattice-2d-glider.ps1 -M 4 -GENS 8 -VACUUM 2 -DEFECT 1 -SEED_R 2 -SEED_C 2 }

# Wave A fabric
Step "N128" { .\flash-capture-phase-lattice-scale.ps1 -N 128 }
Step "M8_smoke" { .\flash-capture-phase-lattice-2d.ps1 -M 8 }

# Wave A endurance (~1e6 cell updates)
Step "soak_62500" { .\flash-capture-phase-lattice-2d-glider.ps1 -M 4 -GENS 62500 -VACUUM 1 -DEFECT 2 -SEED_R 2 -SEED_C 2 }

# Wave B collision milestone
Step "collision_S0" { .\flash-capture-phase-lattice-2d-collision.ps1 -Scenario 0 -GENS 16 }

# Wave C M=8 serial glider
Step "serial_M8" { .\flash-capture-phase-lattice-2d-glider-serial.ps1 -M 8 -GENS 16 }

# Wave D fault
Step "fault_delete" { .\flash-capture-phase-lattice-2d-fault.ps1 -FaultMode 1 -GENS 8 }

"ICE MAX COMPLETE" | Add-Content $log
Write-Host "ICE MAX COMPLETE"
