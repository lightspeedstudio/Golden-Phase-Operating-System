# Resume iCE Max after soak (collision + serial M8 + fault).
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot
$log = Join-Path $PSScriptRoot "evidence\ice_max_run_log.txt"
"RESUME $(Get-Date -Format o)" | Add-Content $log
function Step($name, $script) {
    Write-Host "==== $name ===="
    & $script
    "PASS $name" | Add-Content $log
}
Step "collision_S0" { .\flash-capture-phase-lattice-2d-collision.ps1 -Scenario 0 -GENS 16 }
Step "serial_M8" { .\flash-capture-phase-lattice-2d-glider-serial.ps1 -M 8 -GENS 16 }
Step "fault_delete" { .\flash-capture-phase-lattice-2d-fault.ps1 -FaultMode 1 -GENS 8 }
"ICE MAX COMPLETE" | Add-Content $log
Write-Host "ICE MAX COMPLETE"
