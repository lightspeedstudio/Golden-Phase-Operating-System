# GPOS public release - audit-only unless GPOS_PUBLIC_READY=1
# Audit-only must not: commit, checkout, branch, remote add, push, reset, merge.
$ErrorActionPreference = "Continue"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root
$Report = Join-Path $Root "docs\GPOS_PUBLIC_RELEASE_REPORT.md"
$Ready = $env:GPOS_PUBLIC_READY
$RemoteUrl = "https://github.com/lightspeedstudio/Golden-Phase-Operating-System.git"
$Branch = (git branch --show-current).Trim()
$Sha = (git rev-parse HEAD).Trim()
$Fail = New-Object System.Collections.Generic.List[string]
$Notes = New-Object System.Collections.Generic.List[string]
$GateRows = New-Object System.Collections.Generic.List[string]

function Add-Fail([string]$msg) {
  $script:Fail.Add($msg)
  Write-Host "FAIL $msg"
}

function Test-DenyPath([string]$p) {
  if ([string]::IsNullOrWhiteSpace($p)) { return $false }
  $n = $p -replace '\\', '/'
  if ($n -match '(^|/)scratchpad(/|$)') { return $true }
  if ($n -match '(^|/)zynq_fsbl\.elf$') { return $true }
  if ($n -match '(^|/)system\.bit$') { return $true }
  if ($n -match '(^|/)BOOT\.BIN$') { return $true }
  if ($n -match '(^|/)BOOT\.bin$') { return $true }
  if ($n -match '(^|/)kairos_zynq\.elf$') { return $true }
  if ($n -match 'ed25519$' -and $n -notmatch '\.pub$') { return $true }
  return $false
}

Write-Host "GPOS audit branch=$Branch sha=$Sha ready='$Ready'"

if ($Branch -ne "producing") { Add-Fail "branch is '$Branch', need producing" }

foreach ($f in @("LICENSE.md", "LICENSE-AGPL-3.0.txt", "COMMERCIAL_LICENSE.md", "CONTRIBUTING.md", "COPYRIGHT_NOTICE")) {
  if (-not (Test-Path $f)) { Add-Fail "missing $f" }
}

$cn = if (Test-Path "COPYRIGHT_NOTICE") { Get-Content "COPYRIGHT_NOTICE" -Raw } else { "" }
if ($cn -match '(?i)unpublished' -and $cn -notmatch 'LICENSE\.md') {
  Add-Fail "COPYRIGHT_NOTICE still treats the tree as unpublished"
}

$ssh = Get-Content "src\kernel\net_ssh.rs" -Raw
if ($ssh -notmatch 'AUTHORIZED_PUBKEY:\s*\[u8;\s*32\]\s*=\s*\[0u8;\s*32\]') {
  Add-Fail "AUTHORIZED_PUBKEY is not fail-closed"
} else {
  $Notes.Add("AUTHORIZED_PUBKEY is [0u8; 32]")
}

$cfg = Get-Content ".cargo\config.toml" -Raw
if ($cfg -notmatch '(?m)^\s*linker\s*=\s*"arm-none-eabi-gcc"\s*$') {
  Add-Fail "ARM linker is not PATH arm-none-eabi-gcc"
}
if ($cfg -match '(?m)^\s*linker\s*=\s*".*[Xx]ilinx') {
  Add-Fail "ARM linker is a machine-local Xilinx path"
}

$toml = Get-Content "Cargo.toml" -Raw
foreach ($feat in @("utility", "ssh-bm", "usb", "sched2", "tensor", "cpu1", "craft")) {
  if ($toml -notmatch [regex]::Escape($feat)) { Add-Fail "feature/member token missing: $feat" }
}
if ($toml -notmatch "craft-mainframe") { Add-Fail "workspace member craft-mainframe missing" }

$readme = Get-Content "README.md" -Raw
if ($readme -notmatch 'LICENSE\.md') { Add-Fail "README.md does not point at LICENSE.md" }
if ($readme -match '(?i)(Claude|ChatGPT|Cursor).{0,40}co-founder' -and $readme -notmatch '(?i)not co-founders') {
  Add-Fail "README.md still names a tool as co-founder"
}

$tracked = git ls-files
$headHits = @($tracked | Where-Object { Test-DenyPath $_ })
if ($headHits.Count -gt 0) {
  Add-Fail "HEAD tracks deny-listed blobs ($($headHits.Count))"
  $headHits | Select-Object -First 20 | ForEach-Object { $Notes.Add("HEAD deny: $_") }
} else {
  $Notes.Add("HEAD tree has no deny-listed FSBL/bit/BOOT/ELF/scratchpad/private-key names")
}

$scratchHist = git log --all --oneline -- scratchpad
if ($scratchHist) {
  Add-Fail "scratchpad appears in git history"
} else {
  $Notes.Add("scratchpad never committed")
}

$histNames = git log --all --full-history --pretty=format: --name-only -- `
  "*ed25519" "scratchpad" "scratchpad/**" `
  "zynq_fsbl.elf" "**/zynq_fsbl.elf" `
  "system.bit" "**/system.bit" `
  "BOOT.BIN" "**/BOOT.BIN" "BOOT.bin" "**/BOOT.bin" `
  "kairos_zynq.elf" "**/kairos_zynq.elf"
$histHits = @($histNames | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Sort-Object -Unique)
if ($histHits.Count -gt 0) {
  Add-Fail "history contains FSBL/bit/BOOT/ELF/key/scratchpad paths ($($histHits.Count) names); git rm --cached does not purge them; stop release"
  $histHits | Select-Object -First 15 | ForEach-Object { $Notes.Add("history name: $_") }
}

$revTips = @()
foreach ($tip in @("producing", "main", "HEAD")) {
  git rev-parse --verify --quiet $tip | Out-Null
  if ($LASTEXITCODE -eq 0) { $revTips += $tip }
}
$objHits = New-Object System.Collections.Generic.HashSet[string]
git rev-list --objects @revTips | ForEach-Object {
  $line = $_.Trim()
  if ($line -match '^[0-9a-f]{40}\s+(.+)$') {
    $p = $Matches[1]
    if (Test-DenyPath $p) { [void]$objHits.Add($p) }
  }
}
if ($objHits.Count -gt 0) {
  Add-Fail "rev-list --objects deny-list hits ($($objHits.Count)) on producing/main"
  $objHits | Select-Object -First 15 | ForEach-Object { $Notes.Add("object deny: $_") }
}

$pemNeedle = "BEGIN OPENSSH" + " PRIVATE KEY"
$pemFiles = git log --all --full-history --pretty=format: --name-only -G $pemNeedle --
$pemHits = @($pemFiles | Where-Object {
  $_ -and
  $_ -notmatch '(^|/)(scripts/gpos_public_release\.ps1|docs/GPOS_PUBLIC_RELEASE\.md)$' -and
  $_ -notmatch '(^|/)third_party/' -and
  $_ -notmatch '\.(exe|elf|bin|o)$'
} | Sort-Object -Unique)
if ($pemHits.Count -gt 0) {
  Add-Fail ("history has OpenSSH PEM header in non-parser paths: " + ($pemHits -join ", "))
} else {
  $Notes.Add("no OpenSSH private-key PEM in non-parser text history")
}

$envHist = git log --all --full-history --pretty=format: --name-only -- ".env" "*.env"
$envHits = @($envHist | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Sort-Object -Unique)
if ($envHits.Count -gt 0) {
  Add-Fail ".env paths exist in git history"
}

$gcc = Get-Command arm-none-eabi-gcc -ErrorAction SilentlyContinue
if (-not $gcc) {
  foreach ($d in @(
      "D:\Xilinx\2026.1\Vitis\gnu\aarch32\nt\gcc-arm-none-eabi\bin",
      "D:\Xilinx\2026.1\gnu\aarch32\nt\gcc-arm-none-eabi\bin"
    )) {
    if (Test-Path (Join-Path $d "arm-none-eabi-gcc.exe")) {
      $env:PATH = "$d;" + $env:PATH
      $Notes.Add("audit PATH prepended $d; .cargo/config.toml still uses linker name arm-none-eabi-gcc")
      break
    }
  }
}
rustup target add armv7a-none-eabi 2>$null | Out-Null

function Invoke-Gate([string]$name, [string[]]$argv) {
  Write-Host "GATE $name : $($argv -join ' ')"
  & $argv[0] @($argv[1..($argv.Length - 1)])
  $code = $LASTEXITCODE
  if ($code -ne 0) {
    Add-Fail "gate failed: $name (exit $code)"
    $script:GateRows.Add("| ``$name`` | FAIL exit $code |")
  } else {
    $script:GateRows.Add("| ``$name`` | PASS |")
    $script:Notes.Add("gate pass: $name")
  }
}

Invoke-Gate "cargo-test-default" @("cargo", "test", "--locked")
Invoke-Gate "kairos_zynq-utility" @(
  "cargo", "build", "--locked", "--release", "--bin", "kairos_zynq",
  "--no-default-features", "--features", "zynq-a9,utility",
  "--target", "armv7a-none-eabi"
)
Invoke-Gate "craft-mainframe-build" @("cargo", "build", "-p", "craft-mainframe", "--locked")
Invoke-Gate "craft-mainframe-test" @("cargo", "test", "-p", "craft-mainframe", "--locked")

$when = Get-Date -Format o
$failMd = if ($Fail.Count -eq 0) { "- (none)" } else { ($Fail | ForEach-Object { "- $_" }) -join "`n" }
$noteMd = if ($Notes.Count -eq 0) { "- (none)" } else { ($Notes | ForEach-Object { "- $_" }) -join "`n" }
$gateMd = if ($GateRows.Count -eq 0) { "| (none) | |" } else { $GateRows -join "`n" }
$readyLabel = if ([string]::IsNullOrWhiteSpace($Ready)) { "unset" } else { $Ready }
$status = if ($Fail.Count -eq 0) { "AUDIT PASS - publication still requires GPOS_PUBLIC_READY=1" } else { "AUDIT FAIL - do not publish" }

@"
# GPOS public release report

**Release status:** $status
**Repo:** 1 of 3 - Golden Phase Operating System (this Git repository is GPOS; crate name ``kairos``).
**Certified SHA:** ``$Sha``
**Branch:** ``$Branch``
**GPOS_PUBLIC_READY:** $readyLabel
**When:** $when
**Push:** not performed by audit-only; publish mutates git only after READY=1 and zero failures.

## License checks

| Artifact | Status |
|----------|--------|
| ``LICENSE.md`` | dual AGPLv3-only Path A + commercial Path B |
| ``LICENSE-AGPL-3.0.txt`` | GNU AGPLv3 text |
| ``COMMERCIAL_LICENSE.md`` | Path B instrument pointer |
| ``CONTRIBUTING.md`` | CLA |
| ``COPYRIGHT_NOTICE`` | ownership / docket; public distribution is LICENSE.md |
| ``src/core/gphase_core.rs`` | not modified by this release script |
| Fail-closed SSH | ``AUTHORIZED_PUBKEY = [0u8; 32]`` required |

## Feature inventory

Required tokens remain in ``Cargo.toml``: utility, ssh-bm, usb, sched2, tensor, cpu1, craft, workspace member products/craft-mainframe. Audit must not drop them to pass.

## HEAD / history / secrets

$noteMd

## Build / test results

| Gate | Result |
|------|--------|
$gateMd

## Failures

$failMd

## Publish policy

``git rm --cached`` does not rewrite history. If history contains FSBL, ``system.bit``, ``BOOT.BIN``, ``kairos_zynq.elf``, scratchpad, or private keys, stop. No squash, no ``git init`` replacement, no force-push.

Until ``GPOS_PUBLIC_READY=1`` and this SHA still matches ``producing`` HEAD, production readiness is **not** claimed.
"@ | Set-Content -Path $Report -Encoding utf8

if ($Fail.Count -gt 0) {
  Write-Host "AUDIT FAIL count=$($Fail.Count) sha=$Sha"
  exit 1
}

if ($Ready -ne "1") {
  Write-Host "AUDIT PASS. Not publishing (GPOS_PUBLIC_READY unset). No git mutations."
  exit 0
}

# Publish: SHA lock - HEAD must still be the SHA recorded in this run's report.
$headNow = (git rev-parse HEAD).Trim()
if ($headNow -ne $Sha) {
  Write-Host "SHA changed during audit certified=$Sha head=$headNow"
  exit 1
}
$cert = $null
Select-String -Path $Report -Pattern '^(\*\*)?Certified SHA:(\*\*)?\s*`?([0-9a-f]{40})' | ForEach-Object {
  $cert = $_.Matches[0].Groups[3].Value
}
if ($cert -ne $Sha) {
  Write-Host "SHA mismatch certified=$cert head=$Sha"
  exit 1
}
git merge-base --is-ancestor main $Sha
if ($LASTEXITCODE -ne 0) {
  Write-Host "main cannot fast-forward to $Sha"
  exit 1
}
if (Test-Path $Report) {
  git stash push -m "gpos-audit-report" -- $Report | Out-Null
}
git checkout main
if ($LASTEXITCODE -ne 0) { Write-Host "checkout main failed"; exit 1 }
git merge --ff-only $Sha
if ($LASTEXITCODE -ne 0) { Write-Host "ff-only merge failed"; exit 1 }
git stash pop 2>$null | Out-Null
$origin = git remote get-url origin 2>$null
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($origin)) {
  git remote add origin $RemoteUrl
  if ($LASTEXITCODE -ne 0) { Write-Host "git remote add failed"; exit 1 }
} elseif ($origin.Trim() -ne $RemoteUrl) {
  Write-Host "origin is '$origin', expected $RemoteUrl"
  exit 1
}
git push -u origin main
if ($LASTEXITCODE -ne 0) { Write-Host "git push failed"; exit 1 }
Write-Host "PUBLISHED $Sha to origin main"
exit 0
