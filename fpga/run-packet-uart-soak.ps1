# P10: multi-round live I/M/A/P UART endurance (reuses last PROP_ARM bitstream).
param([int]$Rounds = 5, [int]$GENS = 2)
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot
& .\flash-capture-phase-lattice-2d-packet.ps1 -GENS $GENS -Rounds $Rounds -SkipBuild
if ($LASTEXITCODE -ne 0) { throw "soak failed" }
"PASS uart_soak rounds=$Rounds" | Add-Content evidence\ice_max_run_log.txt
Write-Host "P10 SOAK COMPLETE rounds=$Rounds"
