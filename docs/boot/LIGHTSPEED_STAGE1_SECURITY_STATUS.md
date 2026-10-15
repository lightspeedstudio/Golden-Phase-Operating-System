<!-- Copyright LightSpeed Studio LLC -->
<!-- SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial -->

# LightSpeed Studio LLC — Stage 1 security status

Does not start Systems 13–15. Production provisioning: [`DEFERRED_PRODUCTION_SECURITY_HARDWARE.md`](DEFERRED_PRODUCTION_SECURITY_HARDWARE.md).

## Layers

| Layer | Meaning | Examples |
| --- | --- | --- |
| Implemented | Source exists in `zynq_stage1/owned` | packer, parser, ADMA, DevCfg driver, software RSA, PCAP, handoff |
| Host-verified | `cargo test --features std` / hash locks | SYS02 parse, SYS07 RSA KATs, SYS11 classifier |
| Board-proven | Raw COM12 UART | SYS03–SYS05 HOLD, SYS08 PL PASS, SYS09/SYS10A/SYS12 `gpos>` |
| Powered-session proven | BBRAM programmed this session; no cold-power claim | SYS06 `P1 KAT PASS` |
| Production-deferred | Custom hardware + key ceremony required | Systems 13–15 |

## Recorded capabilities

- Software RSA later-partition verification (SYS07 HOLD; SYS10A UART `STAGE1 RSA2048 PARTITION PASS implementation=SOFTWARE`).
- BBRAM AES/HMAC KAT (SYS06 UART `P1 KAT PASS`; HASHES: `cold_power_bbram_retention: not claimed`).
- Owned secure packer (host; keys gitignored `*.nky`).
- Corrupted-data negative tests: SYS08 `PL FAIL` / `PL FAIL class=CRC`; SYS11 rejects fail/stale/reordered/host-injected fixtures; SYS07 signature fail-closed docs.
- BootROM RSA status currently **not** eFuse-enforced (`NOT_ENFORCED` on SYS10A UART).
- No production PPK hash programmed.
- No permanent AES key (no AES eFuse; research BBRAM only).
- No proven `VCCBATT` retention on PZ-StarLite.
- Systems 13–15: `DEFERRED_CUSTOM_HARDWARE`.

## Explicit non-claims

BootROM RSA eFuse enforcement is not claimed. BBRAM is not claimed power-retained. 29836-byte PT_LOAD is **SYS10_OVERSIZE**, not an 8 KiB PASS.
