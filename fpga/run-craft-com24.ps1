# P8: host craft twin + live COM24 I/M/A/P. Re-flash packet bitstream first.
param([string]$Com = "COM24")
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot
& .\flash-capture-phase-lattice-2d-packet.ps1 -GENS 2 -Rounds 1
if ($LASTEXITCODE -ne 0) { throw "live PROP_ARM flash/capture failed" }

Push-Location (Join-Path $PSScriptRoot "..")
cargo test --test phase_transport_tests one_packet_prop_arm_host_twin -- --nocapture
if ($LASTEXITCODE -ne 0) { throw "host twin failed" }
cargo test --test phase_transport_tests collision_policy_sealed_and_gated -- --nocapture
if ($LASTEXITCODE -ne 0) { throw "collision policy failed" }
Pop-Location

$ev = Join-Path $PSScriptRoot "evidence\fpga_uart_craft_com24"
New-Item -ItemType Directory -Force -Path $ev | Out-Null
Copy-Item (Join-Path $PSScriptRoot "evidence\phase_lattice_2d_packet_prop_arm_G2\uart_capture.txt") (Join-Path $ev "uart_capture.txt") -Force
$hash = Get-Content (Join-Path $PSScriptRoot "evidence\phase_lattice_2d_packet_prop_arm_G2\icebreaker.bin.sha256") -Raw
@(
    "PHYSICALLY_VERIFIED=YES"
    "GATE=fpga_uart_craft_com24"
    "BOARD=iCEBreaker V1.1a"
    "UART=$Com 115200 8N1"
    "ADAPTER=FpgaGliderTransport host twin + live I/M/A/P"
    "FEATURE=fpga-uart"
    "PACKET_BITSTREAM_SHA256=$($hash.Split()[0])"
    "RESULT=craft-over-COM24 PROP_ARM PASS; LocalCraftTransport remains fallback"
) | Set-Content (Join-Path $ev "PHYSICALLY_VERIFIED.txt")
"PASS craft_com24" | Add-Content (Join-Path $PSScriptRoot "evidence\ice_max_run_log.txt")
Write-Host "RESULT: PHYSICALLY_VERIFIED fpga_uart_craft_com24"
