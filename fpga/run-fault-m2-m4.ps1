# Seal fault modes 2..4 (mode 1 already sealed).
param([int]$GENS = 8)
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot
$log = Join-Path $PSScriptRoot "evidence\fault_m2_m4_log.txt"
"FAULT M2-M4 $(Get-Date -Format o)" | Set-Content $log
foreach ($m in 2, 3, 4) {
    Write-Host "===== FAULT MODE $m ====="
    & .\flash-capture-phase-lattice-2d-fault.ps1 -FaultMode $m -GENS $GENS
    if ($LASTEXITCODE -ne 0) { throw "fault mode $m failed" }
    "PASS fault_M$m" | Add-Content $log
}
"FAULT M2-M4 COMPLETE" | Add-Content $log
Write-Host "FAULT M2-M4 COMPLETE"
