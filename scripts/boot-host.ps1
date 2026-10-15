# KairOS hosted runtime — stdin shell + VM programs
$ErrorActionPreference = "Stop"
Set-Location (Split-Path $PSScriptRoot -Parent)
$cargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"

if ($args.Count -gt 0) {
    & $cargo run --bin kairos_host -- @args
} else {
    Write-Host "Golden Phase Operating System — interactive shell at gpos>"
    Write-Host "Try: help | whoami | host | ps | mem | run hello | log | exit"
    & $cargo run --bin kairos_host -- --interactive
}
