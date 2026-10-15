# Seal collision scenarios 1..3 sequentially (S0 already sealed).
param([int]$GENS = 16)
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot
$log = Join-Path $PSScriptRoot "evidence\collision_s1_s3_log.txt"
"COLLISION S1-S3 $(Get-Date -Format o)" | Set-Content $log
foreach ($s in 1, 2, 3) {
    Write-Host "===== SCENARIO $s ====="
    & .\flash-capture-phase-lattice-2d-collision.ps1 -Scenario $s -GENS $GENS
    if ($LASTEXITCODE -ne 0) { throw "scenario $s failed" }
    "PASS collision_S$s" | Add-Content $log
}
"COLLISION S1-S3 COMPLETE" | Add-Content $log
Write-Host "COLLISION S1-S3 COMPLETE"
