# Avenue 9 -- FPGA lattice scaling study. Synth + PnR only (NO flashing).
# Loops the 1-D ring over N in $Ns (chparam override of phase_lattice_scale_top)
# and, if -Include2D, the 2-D torus over M in $Ms.
# Writes evidence/phase_lattice_scale/N<val>/{yosys.log,nextpnr.log,report.json}
# and evidence/phase_lattice_scale/2d_M<val>/{...} plus a summary.csv.
param(
    [int[]]$Ns = @(16, 32, 64, 128, 256),
    [int[]]$Ms = @(4, 8),
    [switch]$Skip1D,
    [switch]$Skip2D
)
$ErrorActionPreference = "Continue"
$CAD = "C:\Tools\oss-cad-suite\oss-cad-suite"
if (-not (Test-Path "$CAD\bin\yosys.exe")) { throw "OSS CAD Suite not found at $CAD" }
$env:PATH = "$CAD\bin;$CAD\lib;$env:PATH"
Set-Location $PSScriptRoot

$evRoot = Join-Path $PSScriptRoot "evidence\phase_lattice_scale"
New-Item -ItemType Directory -Force -Path $evRoot | Out-Null
$summaryPath = Join-Path $evRoot "summary.csv"
"config,lc_used,lc_total,fmax_mhz,fits,build_ok" | Set-Content $summaryPath

function Get-LastFmax($log) {
    $m = Select-String -Path $log -Pattern "Max frequency for clock 'clk\$SB_IO_IN_\$glb_clk':\s*([\d.]+) MHz" -AllMatches
    if ($m.Matches.Count -eq 0) {
        $m2 = Select-String -Path $log -Pattern "Max frequency for clock '.*':\s*([\d.]+) MHz" -AllMatches
        if ($m2.Count -eq 0) { return $null }
        return [double]($m2[-1].Matches[0].Groups[1].Value)
    }
    $last = $m[-1]
    return [double]($last.Matches[0].Groups[1].Value)
}

function Get-LC($log) {
    $m = Select-String -Path $log -Pattern "ICESTORM_LC:\s*(\d+)/\s*(\d+)"
    if ($m.Count -eq 0) { return $null }
    return @{ used = [int]$m[0].Matches[0].Groups[1].Value; total = [int]$m[0].Matches[0].Groups[2].Value }
}

if (-not $Skip1D) {
    foreach ($n in $Ns) {
        $tag = "N$n"
        Write-Host "==================== 1-D ring $tag ===================="
        $ev = Join-Path $evRoot $tag
        New-Item -ItemType Directory -Force -Path $ev | Out-Null
        $yosysLog = Join-Path $ev "yosys.log"
        $json = Join-Path $ev "out.json"
        $asc = Join-Path $ev "out.asc"
        $pnrLog = Join-Path $ev "nextpnr.log"
        $report = Join-Path $ev "report.json"

        & yosys -l $yosysLog -p "read_verilog uart_tx.v phase_lattice_uart.v phase_lattice_uart_scale.v; chparam -set N $n phase_lattice_scale_top; synth_ice40 -top phase_lattice_scale_top -json $json"
        $yosysOk = ($LASTEXITCODE -eq 0) -and (Test-Path $json)
        $buildOk = $false; $lcUsed = ""; $lcTotal = ""; $fmax = ""; $fits = "NO"
        if ($yosysOk) {
            cmd /c "nextpnr-ice40 --up5k --package sg48 --json `"$json`" --pcf icebreaker.pcf --asc `"$asc`" --report `"$report`" > `"$pnrLog`" 2>&1"
            $pnrOk = ($LASTEXITCODE -eq 0) -and (Test-Path $asc)
            $lc = Get-LC $pnrLog
            if ($null -ne $lc) { $lcUsed = $lc.used; $lcTotal = $lc.total }
            $fmax = Get-LastFmax $pnrLog
            if ($pnrOk) { $buildOk = $true; $fits = "YES" }
        } else {
            Write-Host "YOSYS FAILED for $tag"
        }
        Write-Host "$tag : LC=$lcUsed/$lcTotal Fmax=$fmax MHz fits=$fits build_ok=$buildOk"
        "$tag,$lcUsed,$lcTotal,$fmax,$fits,$buildOk" | Add-Content $summaryPath
    }
}

if (-not $Skip2D) {
    foreach ($m in $Ms) {
        $tag = "2d_M$m"
        Write-Host "==================== 2-D torus $tag ===================="
        $ev = Join-Path $evRoot $tag
        New-Item -ItemType Directory -Force -Path $ev | Out-Null
        $yosysLog = Join-Path $ev "yosys.log"
        $json = Join-Path $ev "out.json"
        $asc = Join-Path $ev "out.asc"
        $pnrLog = Join-Path $ev "nextpnr.log"
        $report = Join-Path $ev "report.json"

        & yosys -l $yosysLog -p "read_verilog uart_tx.v phase_lattice_uart.v phase_lattice_2d_scale.v; chparam -set M $m phase_lattice_2d_top; synth_ice40 -top phase_lattice_2d_top -json $json"
        $yosysOk = ($LASTEXITCODE -eq 0) -and (Test-Path $json)
        $buildOk = $false; $lcUsed = ""; $lcTotal = ""; $fmax = ""; $fits = "NO"
        if ($yosysOk) {
            cmd /c "nextpnr-ice40 --up5k --package sg48 --json `"$json`" --pcf icebreaker.pcf --asc `"$asc`" --report `"$report`" > `"$pnrLog`" 2>&1"
            $pnrOk = ($LASTEXITCODE -eq 0) -and (Test-Path $asc)
            $lc = Get-LC $pnrLog
            if ($null -ne $lc) { $lcUsed = $lc.used; $lcTotal = $lc.total }
            $fmax = Get-LastFmax $pnrLog
            if ($pnrOk) { $buildOk = $true; $fits = "YES" }
        } else {
            Write-Host "YOSYS FAILED for $tag"
        }
        Write-Host "$tag : LC=$lcUsed/$lcTotal Fmax=$fmax MHz fits=$fits build_ok=$buildOk"
        "$tag,$lcUsed,$lcTotal,$fmax,$fits,$buildOk" | Add-Content $summaryPath
    }
}

Write-Host "==================== SUMMARY ===================="
Get-Content $summaryPath
