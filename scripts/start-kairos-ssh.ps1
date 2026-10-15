# Start KairOS hosted SSH server (default 127.0.0.1:2222, user kairos, Ed25519 keys)
$ErrorActionPreference = "Stop"
Set-Location (Split-Path $PSScriptRoot -Parent)
$cargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"

$port = 2222
$listen = "127.0.0.1"
$allowPassword = $false
$extra = @()

foreach ($arg in $args) {
    if ($arg -eq "--allow-password") { $allowPassword = $true; $extra += $arg; continue }
    if ($arg -eq "--port" -or $arg -eq "--listen") { $extra += $arg; continue }
}
if ($extra -contains "--port") {
    $idx = [array]::IndexOf($extra, "--port")
    if ($idx -ge 0 -and $idx + 1 -lt $extra.Count) { $port = [int]$extra[$idx + 1] }
}
if ($extra -contains "--listen") {
    $idx = [array]::IndexOf($extra, "--listen")
    if ($idx -ge 0 -and $idx + 1 -lt $extra.Count) { $listen = $extra[$idx + 1] }
}

Write-Host "Building kairos_ssh..."
& $cargo build --bin kairos_ssh
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$dataDir = Join-Path $env:USERPROFILE ".kairos"
if (-not (Test-Path $dataDir)) { New-Item -ItemType Directory -Path $dataDir | Out-Null }

Write-Host "KairOS SSH server -> ${listen}:${port} (user kairos)"
Write-Host "Host key: $dataDir\ssh\ssh_host_ed25519_key"
Write-Host "Authorized keys: $dataDir\ssh\authorized_keys"
if ($allowPassword) {
    Write-Host "Password auth enabled (set KAIROS_SSH_PASSWORD)"
}

& $cargo run --bin kairos_ssh -- @extra
