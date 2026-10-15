# Physical Seal Index — SHAs and Paths

**Docket:** CAREGAGA-GPOS-2026-001  
**Board:** PZ7020 · UART COM12 @ 115200  
**Inventor records date:** 2026-07-30  

Paths relative to `kairos/` unless noted.

---

## 1. G64 physical gate — `run_20260730_175025`

**Directory:** `fpga/zynq/evidence/kairos_zynq_boot/run_20260730_175025/`

| File | SHA-256 |
|------|---------|
| `transcript.txt` | `10db227cabbcf6dbd5730163dfc4efe9180c96a31c4112a9ec9bb51dd2e074d0` |
| `results.json` | `5b7fb251b0c9ed2d8e32671b34e4d27ab15096011da12d0b5a79a7a9faaef8c5` |
| `report.md` | `ddeb9cc28695f71fa278f903ba7146ea0a8a5486d50523807e4a5498c9e4c7a2` |

**Results (PHYSICALLY VERIFIED):**

| Check | Result |
|-------|--------|
| G1P @ `0x40000000` | ID `0x47504C34` / ver `0x00020000` |
| G64 @ `0x40001000` | ID `0x47363453` (`G64S`) / ver `0x00010000` |
| `g64 compose` | PASS **16/16** |
| `g64 reflect` | PASS **4/4** |
| `g64 atlas` | PASS **identity=52 reflection=12 error=0** (`hw==gphase_core`) |
| `g64 benchmark` | **8/8** atlas iterations |
| Shell live after ops | PASS |

Reports: `docs/research/G64_PHYSICAL_COM12_REPORT_20260730.md`

---

## 2. Feature seal — `run_20260730_151124`

**Directory:** `fpga/zynq/evidence/kairos_zynq_boot/run_20260730_151124/`

| File | SHA-256 |
|------|---------|
| `transcript.txt` | `8e9ea7957ebddb40b1a42c93507158f5c78b64efc59fe8d4b0e33b6300830429` |
| `results.json` | `34671917ac9bd5f24204ebfde802dfebce20cb3a325c37733b0691519605739f` |
| `report.md` | `00fb6f1f6845aad48a1b6fd45b4dcfb44929cd68eff67bbc69d11ac67dcaeb5c` |

Doc: `docs/PZ7020_FEATURE_SEAL_20260730.md`

---

## 3. GEM ladder — `run_20260730_155025`

**Directory:** `fpga/zynq/evidence/kairos_zynq_boot/run_20260730_155025/`

| File | SHA-256 |
|------|---------|
| `transcript.txt` | `d774082879f439da988116b7c564e0de4433bcc3961a240f7d727fc9c87f5cc7` |
| `results.json` | `d0c4eaf6a60b7a3f5acce93f0b61d3743d9c956d77ae054d4bd3f2ac9d189a8d` |
| `report.md` | `68782648af1c1e888823ae63503bb9e6e0b832662628a0ba0da3e44f0c8865c9` |

---

## 4. Feature BOOT image pack

**Directory:** `fpga/zynq/evidence/kairos_zynq_boot/BOOT_PZ7020_FEATURE_TEST/`

| File | SHA-256 |
|------|---------|
| `BOOT_PZ7020_FEATURE_TEST.BIN` | `e17a3ac5b16deac3688bd93cde6048f583b680ebc6b8099edc701682d1e8344b` |
| `kairos_zynq.elf` | `7061492d9d76bcb64e8a41f4ab6dbeaa4239cf339f992e072171cfdeaba850d1` |

Restore package: `fpga/zynq/research/g64_phase_fabric_sidecar/RESTORE_DEFAULT_BOOT/` (same feature BOOT SHA).

---

## 5. G64 research bitstream

**Directory:** `fpga/zynq/research/g64_phase_fabric_sidecar/release/PZ7020_G64_SIDECAR/`

| File | SHA-256 |
|------|---------|
| `system_g64_sidecar.bit` | `EC3D4FA684D20480C83C655F12161EBAB323F29FC6DB31C2E8EB177CCD2325FE` |

---

## 6. Host oracle (G64 research emit)

**Directory:** `fpga/zynq/research/g64_phase_fabric_sidecar/oracle/`

| File | SHA-256 |
|------|---------|
| `golden_phase_oracle.bin` | `99eca89134347850322269f6c6e55d6ab3f7dd1cdaa1237988c6c5b94448183b` |
| `golden_phase_oracle.json` | `c8a31184e90454ecffb7b3c751635cf1870d4b2ea0bd028405b7156f083a7645` |
| `golden_phase_oracle.md` | `a0a22d214b5b5a4487c95b815595bfc9e1f49147b5a78c1882935852a11356d7` |

`SOURCE_COMMIT` recorded in oracle `SHA256SUMS.txt` (historical emit commit).

Census: identity **52**, reflection **12**, error **0**.

---

## 7. Related console-proof tag (repo)

| Tag | Note |
|-----|------|
| `pz7020-console-proof-20260730` | Feature console proof lineage |
| `g64-sidecar-proof-20260730` | Planned / create if not yet tagged on research tip |

---

## 8. Inventor note

Keep offline copies of SD images and UART captures with this index. Do not overwrite sealed transcripts (`com12_capture_20260730_132633.txt` and dated `run_*` folders).
