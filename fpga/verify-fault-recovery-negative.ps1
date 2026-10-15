# verify-fault-recovery-negative.ps1
# Negative-test gate: proves phase_lattice_2d_fault performs REAL executable
# recovery (live lattice restored to the authenticated checkpoint) and that a
# print-only recovery (RECOVER=observed, no register restore) CANNOT pass.
#
# Compiles + simulates tb_fault_recovery_negative.v with Icarus Verilog for each
# fault mode and fails the build if any mode does not achieve real restoration.
$ErrorActionPreference = "Stop"
$IV = "C:\iverilog\bin\iverilog.exe"
$VP = "C:\iverilog\bin\vvp.exe"
if (-not (Test-Path $IV)) { throw "iverilog not found at $IV" }
Set-Location $PSScriptRoot

$srcs = @("uart_tx.v", "phase_lattice_uart.v", "phase_lattice_2d_fault.v", "tb_fault_recovery_negative.v")
$modes = @(0, 1, 2, 3, 4)
$fail = 0
foreach ($m in $modes) {
    $out = Join-Path $env:TEMP "neg_$m.out"
    & $IV -g2012 -D "FAULT_MODE_SEL=$m" -o $out @srcs 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) { Write-Host "COMPILE FAIL mode $m"; $fail++; continue }
    $log = & $VP $out 2>&1
    $result = ($log | Select-String "RESULT:").ToString()
    Write-Host $result
    if ($result -notmatch "RESULT: PASS") { $fail++ }
}
if ($fail -ne 0) { Write-Host "NEGATIVE TEST FAILED ($fail mode(s)) - recovery is not provably real"; exit 1 }
Write-Host "NEGATIVE TEST PASS - phase_lattice_2d_fault recovery is REAL for all modes [HOST_UNIT_TEST]"
exit 0
