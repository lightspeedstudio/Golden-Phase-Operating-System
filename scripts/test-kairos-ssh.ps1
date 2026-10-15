# SSH unit smoke + optional live ssh client check
$ErrorActionPreference = "Stop"
Set-Location (Split-Path $PSScriptRoot -Parent)
$cargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"

Write-Host "Running SSH server tests..."
& $cargo test --test ssh_server_tests -- --nocapture 2>&1 | Tee-Object -Variable testOut
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
if ($testOut -notmatch "KAIROS SSH SMOKE OK") {
    Write-Error "Missing KAIROS SSH SMOKE OK marker"
    exit 1
}
Write-Host "KAIROS SSH TESTS OK"

$sshExe = Get-Command ssh -ErrorAction SilentlyContinue
if (-not $sshExe) {
    Write-Host "OpenSSH client not found; skipping live connection test"
    exit 0
}

$testRoot = Join-Path $env:TEMP "kairos-ssh-live-$([guid]::NewGuid().ToString('N'))"
New-Item -ItemType Directory -Path $testRoot -Force | Out-Null
$keyPath = Join-Path $testRoot "id_ed25519"
$env:KAIROS_DATA_DIR = $testRoot
$livePort = 2244

ssh-keygen -t ed25519 -f $keyPath -N '""' -q
$pubLine = Get-Content "$keyPath.pub" -Raw

# Register key via library path: write minimal registry consumed by server
$sshDir = Join-Path $testRoot "ssh"
New-Item -ItemType Directory -Path $sshDir -Force | Out-Null
$fp = (& ssh-keygen -lf "$keyPath.pub" -E sha256).Split()[1]
$registry = "fingerprint=$fp;identity=live-smoke;authority=operator;enabled=true;created=1;last_used=-;expires=-;line=$($pubLine.Trim())"
Set-Content -Path (Join-Path $sshDir "key_registry.json") -Value $registry -NoNewline
Set-Content -Path (Join-Path $sshDir "authorized_keys") -Value $pubLine.Trim()

$server = Start-Process -FilePath $cargo -ArgumentList @(
    "run", "--quiet", "--bin", "kairos_ssh", "--", "--port", "$livePort", "--listen", "127.0.0.1"
) -PassThru -NoNewWindow -WorkingDirectory (Get-Location)
Start-Sleep -Seconds 4

try {
    $out = & ssh -i $keyPath -p $livePort -o StrictHostKeyChecking=no -o UserKnownHostsFile=NUL -o BatchMode=yes kairos@127.0.0.1 "help" 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Host $out
        Write-Error "Live ssh help command failed"
        exit 1
    }
    if ($out -notmatch "spawn") {
        Write-Host $out
        Write-Error "Live ssh output missing help text"
        exit 1
    }
    Write-Host "KAIROS SSH LIVE OK"
}
finally {
    if ($server -and -not $server.HasExited) { Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue }
    Remove-Item -Recurse -Force $testRoot -ErrorAction SilentlyContinue
}
