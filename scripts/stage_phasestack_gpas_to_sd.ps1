# Copy signed hello + board negative GPAs to an SD FAT root as 8.3 names.
# Negatives are packed in TEMP (no SK in the repo).
param(
    [Parameter(Mandatory = $true)]
    [string]$SdRoot
)
$ErrorActionPreference = "Stop"
$Kairos = Split-Path -Parent $PSScriptRoot
$Hello = Join-Path $Kairos "fpga\zynq\release\phasestack\hello.gpa"
$Unk = Join-Path $env:TEMP "kairos_gpa_unknown_v1\unk.gpa"
$Old = Join-Path $env:TEMP "kairos_gpa_retired_v1\old.gpa"
$Bad = Join-Path $env:TEMP "kairos_gpa_bad_v1\bad.gpa"
foreach ($p in @($Hello, $Unk, $Old, $Bad)) {
    if (-not (Test-Path $p)) { throw "missing $p" }
}
Copy-Item $Hello (Join-Path $SdRoot "HELLO.GPA") -Force
Copy-Item $Bad (Join-Path $SdRoot "BAD.GPA") -Force
Copy-Item $Unk (Join-Path $SdRoot "UNK.GPA") -Force
Copy-Item $Old (Join-Path $SdRoot "OLD.GPA") -Force
Get-ChildItem $SdRoot\*.GPA | ForEach-Object {
    $h = (Get-FileHash -Algorithm SHA256 $_.FullName).Hash
    Write-Host "$h  $($_.Name) $($_.Length)"
}
