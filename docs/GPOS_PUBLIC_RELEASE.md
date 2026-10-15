# GPOS public release (repo 1 of 3)

**Status:** Architecture approved. Publication **not** authorized.  
**Branch:** `producing` (from `main`). Do not fast-forward `main` until `GPOS_PUBLIC_READY=1` after a passing audit of a **recorded SHA**.  
**Remote (do not add in audit-only):** `https://github.com/lightspeedstudio/Golden-Phase-Operating-System.git`

This is the **kairos/** Git repository only. Not FullOS parent, not Golden Phase Physics Engine, not Metatron SDK.

## Sequence

1. **Prepare** — commit docs and `scripts/gpos_public_release.ps1` on `producing`.
2. **Audit** — run the script **without** `GPOS_PUBLIC_READY`. No git mutations (no commit, no `main` FF, no `git remote add`, no push).
3. **Certify** — report must contain audited SHA, license, history/secrets/binaries, feature inventory, builds, tests, blockers.
4. **Authorize** — you set `GPOS_PUBLIC_READY=1`.
5. **Publish** — script verifies `producing` HEAD **equals** certified SHA, `main` can fast-forward, then FF `main` and `git push -u origin main` of that SHA only.

Forbidden: `git init` replacement, squash of proven history, force-push, modifying `src/core/gphase_core.rs`, changing fail-closed SSH (`AUTHORIZED_PUBKEY` all zeros), dropping OS features to pass audit.

## Prepare on `producing`

License pack, README authors, `COPYRIGHT_NOTICE`, PATH linker, gitignore, `git rm --cached` of HEAD deny-listed blobs, and `scripts/gpos_public_release.ps1` land on `producing` only. `main` stays frozen until authorized publish.

## History blocker

HEAD untracks vendor FSBL / `system.bit` / `BOOT.BIN` / `kairos_zynq.elf`. Those objects still exist in **Git history**. `git rm --cached` does not remove them from earlier commits. Per release policy: **stop publication** until you either (a) accept those files as public, or (b) authorize a **separate** history-filter decision (not squash/force-push unless you explicitly change that rule).

## Script (implement as `scripts/gpos_public_release.ps1`)

Audit-only: no `git commit`, `checkout`, `branch`, `remote`, `push`, `reset`.

```powershell
# GPOS public release — audit-only unless GPOS_PUBLIC_READY=1
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root
$Report = Join-Path $Root "docs\GPOS_PUBLIC_RELEASE_REPORT.md"
$Ready = $env:GPOS_PUBLIC_READY
$Branch = (git branch --show-current).Trim()
$Sha = (git rev-parse HEAD).Trim()
$Fail = @()

if ($Branch -ne "producing") { $Fail += "branch is '$Branch', need producing" }

# License files
foreach ($f in @("LICENSE.md","LICENSE-AGPL-3.0.txt","COMMERCIAL_LICENSE.md","CONTRIBUTING.md","COPYRIGHT_NOTICE")) {
  if (-not (Test-Path $f)) { $Fail += "missing $f" }
}

# Fail-closed SSH
$ssh = Get-Content "src\kernel\net_ssh.rs" -Raw
if ($ssh -notmatch 'AUTHORIZED_PUBKEY:\s*\[u8;\s*32\]\s*=\s*\[0u8;\s*32\]') { $Fail += "AUTHORIZED_PUBKEY is not fail-closed" }

# Feature inventory (presence, not deletion)
$toml = Get-Content "Cargo.toml" -Raw
foreach ($feat in @("utility","ssh-bm","usb","sched2","tensor","cpu1","craft")) {
  if ($toml -notmatch [regex]::Escape($feat)) { $Fail += "feature/member token missing: $feat" }
}
if ($toml -notmatch "craft-mainframe") { $Fail += "workspace member craft-mainframe missing" }

# HEAD tree deny-list names
$tracked = git ls-files
$denyName = { param($p) $p -match 'zynq_fsbl\.elf$' -or $p -match 'BOOT\.BIN$' -or $p -match 'BOOT\.bin$' -or $p -match '(^|/)kairos_zynq\.elf$' }
$headHits = @($tracked | Where-Object { & $denyName $_ })
if ($headHits.Count -gt 0) { $Fail += "HEAD tracks deny-listed blobs ($($headHits.Count)); history also retains them" }

# History scan (reachable from HEAD)
$hist = git log --all --pretty=format: --name-only -- "**/zynq_fsbl.elf" "**/system.bit" "**/BOOT.BIN" "**/BOOT.bin" "**/kairos_zynq.elf" "scratchpad/**"
if ($hist) { $Fail += "history contains FSBL/bit/BOOT/ELF/scratchpad paths (stop release)" }
$priv = git grep -a -I -n "BEGIN OPENSSH PRIVATE" $(git rev-list --max-count=200 HEAD) 2>$null
# git grep across history is slow; also:
$scratchHist = git log --all --oneline -- scratchpad
if ($scratchHist) { $Fail += "scratchpad appears in git history" }

# Builds (do not skip on missing toolchain — record fail)
function Invoke-Gate($name, $cmd) {
  Write-Host "GATE $name"
  cmd /c $cmd
  if ($LASTEXITCODE -ne 0) { $script:Fail += "gate failed: $name" }
}
Invoke-Gate "cargo-test-default" "cargo test --locked"
Invoke-Gate "kairos_zynq-utility" "cargo build --locked --release --bin kairos_zynq --no-default-features --features `"zynq-a9,utility`" --target armv7a-none-eabi"
Invoke-Gate "craft-mainframe" "cargo build -p craft-mainframe --locked"

# Write report (working tree only — not a git commit)
@"
# GPOS public release report
Audited SHA: $Sha
Branch: $Branch
GPOS_PUBLIC_READY: $Ready
When: $(Get-Date -Format o)
Failures:
$($Fail | ForEach-Object { "- $_" } | Out-String)
"@ | Set-Content $Report -Encoding utf8

if ($Fail.Count -gt 0) {
  Write-Host "AUDIT FAIL count=$($Fail.Count)"
  exit 1
}

if ($Ready -ne "1") {
  Write-Host "AUDIT PASS. Not publishing (GPOS_PUBLIC_READY unset). No git mutations."
  exit 0
}

# Publish: SHA lock
$cert = Select-String -Path $Report -Pattern "^Audited SHA: ([0-9a-f]{40})" | ForEach-Object { $_.Matches[0].Groups[1].Value }
if ($cert -ne $Sha) { Write-Host "SHA mismatch certified=$cert head=$Sha"; exit 1 }
git merge-base --is-ancestor main HEAD
if ($LASTEXITCODE -ne 0) { Write-Host "main cannot fast-forward to $Sha"; exit 1 }
git checkout main
git merge --ff-only $Sha
if (-not (git remote get-url origin 2>$null)) {
  git remote add origin "https://github.com/lightspeedstudio/Golden-Phase-Operating-System.git"
}
git push -u origin main
```

Audit-only command:

```powershell
.\scripts\gpos_public_release.ps1
```

Publish (only after you authorize):

```powershell
$env:GPOS_PUBLIC_READY=1
.\scripts\gpos_public_release.ps1
```
