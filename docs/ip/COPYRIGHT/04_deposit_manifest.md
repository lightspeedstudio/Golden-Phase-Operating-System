# Copyright Deposit Manifest — Unpublished Computer Program

**Docket:** CAREGAGA-GPOS-2026-001  
**Work:** Golden Phase Operating System (Kairos / GPOS)  
**Claimant:** Richard Alberto Careaga  
**Deposit type:** Unpublished source code (+ selected project RTL / docs)

---

## Purpose

Instructions for building a deposit archive for U.S. Copyright Office registration. **Do not auto-upload.** Create the zip locally, compute SHA-256, then upload via copyright.gov.

---

## Include (from `kairos/` repo root)

### A. Core OS / kernel source

- `src/core/` — including `gphase_core.rs` (algebra root)
- `src/kernel/` — scheduler, memory, authority, state_root, craft/FDIR, etc.
- `src/stateclock/`
- `src/vm/`
- `src/isa/`
- `src/fs/` — PhaseFS
- `src/userland/`
- `src/board/zynq7000/` — including `g1p.rs`, `g64.rs`, `kernel_main.rs`, `memory_map.rs`
- `src/zynq_oracle/` — oracle emitter (host tooling)
- `src/bin/` — `kairos_zynq.rs`, `kairos_zynq` research bins, `gpl_oracle.rs`, etc. as present
- `Cargo.toml`, `Cargo.lock` (if tracked)
- `COPYRIGHT_NOTICE`
- `docs/ip/` (this pack)
- Selected architecture docs: `docs/TWO_OS_ARCHITECTURE.md`, `fpga/zynq/GOLDEN_PHASE_ZYNQ_STACK.md`, `docs/research/G64_*.md`, `docs/PZ7020_FEATURE_SEAL_20260730.md`

### B. Selected FPGA RTL (project-authored)

- `fpga/zynq/research/g64_phase_fabric_sidecar/rtl/` (exclude huge Vivado project caches if any)

### C. Evidence indexes (not necessarily full BOOT binaries)

- `docs/ip/EVIDENCE/`
- `fpga/zynq/evidence/kairos_zynq_boot/run_20260730_175025/SHA256SUMS.txt` (and report.md if size allows)
- `fpga/zynq/evidence/kairos_zynq_boot/run_20260730_151124/SHA256SUMS.txt`
- `fpga/zynq/evidence/kairos_zynq_boot/run_20260730_155025/SHA256SUMS.txt`

If USCO size limits block binaries, deposit **source + SHA manifests**; keep full BOOT images offline with the inventor.

---

## Exclude

- Third-party UART tools (SecureCRT, sscom, etc.)
- VMware installers
- Large seller PDF datasheet trees outside kairos
- `target/` build artifacts
- Private keys, ANOM passphrases, `.env` secrets
- Vivado temporary journals / backup logs outside research release

---

## Suggested zip name

```text
CAREGAGA-GPOS-2026-001_unpublished_deposit_YYYYMMDD.zip
```

Record:

| Item | Value |
|------|--------|
| Zip SHA-256 | _______________________________ |
| Date created | _______________ |
| USCO case / service number | _______________ |

---

## Example PowerShell (run from `kairos/`)

```powershell
# Review paths before running; adjust if your tree differs.
$stamp = Get-Date -Format "yyyyMMdd"
$out = "CAREGAGA-GPOS-2026-001_unpublished_deposit_$stamp.zip"
# Prefer manually assembling a staging folder, then:
# Compress-Archive -Path .\deposit_staging\* -DestinationPath $out
Get-FileHash $out -Algorithm SHA256
```
