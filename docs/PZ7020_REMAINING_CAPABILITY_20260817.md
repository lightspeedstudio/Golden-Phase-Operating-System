# PZ7020 remaining capability matrix — 2026-08-17

Audit of the eight leftover items. Allowed statuses only:
`PASS_ON_PZ7020`, `EXISTING_PROOF_BOUND`, `SOFTWARE_MITIGATION`,
`HARDWARE_UNAVAILABLE`, `OPTIONAL_PERFORMANCE_OPEN`, `FAILED_REAL_TEST`.

No fake PASS. Host unit tests are not hardware proof. UART files are
evidence; `*.host.jsonl` is host parser log only.

Board image for new work: `--no-default-features --features "zynq-a9,utility"`.
Gold PL PATH1_G64 `system.bit`
`d19f08d20cac85ea0e33c41a8a552166f5a9176e16977f09aa64d17e68135829`.
Do not rebuild PL. Do not `fsj format`. Sacrificial SD `121220160204`.
COM12 CH340, DTR/RTS off.

## Matrix

| Item | Claim | Status | Proof / blocker |
| --- | --- | --- | --- |
| 1 | Signed GPA + PhaseSentinel E2E (install, map, contain violator, valid app still launches, unmap, shell up) | `PASS_ON_PZ7020` | `BOOT_A24_PHASESENTINEL_E2E_20260817/` UART `com12_a24_20260817_125701.uart`. Contain `over.gpa` then hello relaunches `sentinel=Allow`. |
| 2 | USB hub `214b:7250` + HID behind hub | `PASS_ON_PZ7020` | Direct J10 HID remains A15. Behind hub: `BOOT_A25_HUB_MOUSE_DEBUG_20260817/` UART `com12_a25_hub_20260817_140859.uart` CLASS `USB_HUB_HID`. Mouse `046d:c092` hub=3 port=1 168 reports; keyboard `3151:4015` hub=2 port=4 39 reports. Nested hub depth bound still refuses expand; connected leaves are enumerated. |
| 3 | Ethernet 1G | `PASS_ON_PZ7020` (1G and 100/full) | 100/full: `BOOT_A16_88B5_TXBURST_20260817/` UART n=32 host 32/32. 1G: `BOOT_A28_1G_20260817/` UART `com12_a28_1g_20260818_114435.uart` / `114656.uart` CLASS `WIRE_88B5_32_1G`. `1gprime seq=16` then txburst seq 17–48 intact. Idle 1G still drops the first ~7–12 frames; init primes 16. |
| 4 | UART–GEM PhaseNet bridge | `PASS_ON_PZ7020` | `BOOT_A26_UARTGEM_BRIDGE_20260817/` UART `com12_a26_bridge_20260817_134703.uart`. `phaselink init` then `bridge rx: ack:ping-e2e` ethertype 0x88B5; oversize refused. Splash stays `none` until init. |
| 5 | Interactive SSH PTY | `PASS_ON_PZ7020` | `BOOT_A27_SSH_PTY_20260817/` UART `com12_a27_ssh_20260817_173127.uart` sha256 `e0b24a29…ada92a` CLASS `SSH_PTY`. OpenSSH `-tt` `gpos>` + `fsj format refused: type CONFIRM`; serve `authed=1 shells=1`; bad key `Permission denied`. Runtime `net ssh key` only. Tree `AUTHORIZED_PUBKEY` all zeros. |
| 6 | GPA install through SD WAL + recovered `app run` | `EXISTING_PROOF_BOUND` | A11 `BOOT_A11_GPA_WAL_20260816/` commit `9ad67f1` ELF `ee49d54b…354de362` — GPA files survive SD WAL; recover `app list` empty. A23 `BOOT_A23_APPREGISTRY_20260816/` commit `40aac73` ELF `343182b6…03f58454` UART recover `f7e57d70…fe0e8982` — reconstruct + `app run hello-a7f3c91e` `sentinel=Allow`. Bind only; no rebuild. |
| 7 | IOMMU | `HARDWARE_UNAVAILABLE` + DMA `SOFTWARE_MITIGATION` | PZ7020 has no IOMMU. CPU MMU is not an IOMMU. `dma_contain` host tests 5/5; GEM `wire_tx_raw` refuses out-of-pool descriptors. USB QH/qTD not fully pooled. See `docs/PZ7020_KNOWN_LIMITATIONS.md`. |
| 8 | Hardware TRNG | `HARDWARE_UNAVAILABLE` | Zynq-7000 PS has no TRNG. Source name `TIMER_JITTER_SEEDED_DRBG`. Health tests + `HardwareRequired` fail-closed. See limitations. |

## Software compile state (2026-08-17, this tree)

Host lib tests: `dma_contain` 5/5, `usb_hub` 4/4, `uart_gem` 2/2.

ARM: `cargo build --release --bin kairos_zynq --no-default-features --features "zynq-a9,utility" --target armv7a-none-eabi` succeeded. That is not board proof. Board proof for items 1, 2, 3, 4, and 5 is the UART files named in the matrix.

## Not claimed

No push. No IOMMU. No TRNG.
