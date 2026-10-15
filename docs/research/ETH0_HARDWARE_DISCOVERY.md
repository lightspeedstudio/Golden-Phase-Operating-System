# ETH-0 — PZ7020 GEM0 / RTL8211F Hardware Discovery

**Gate:** ETH-0 (stop gate — documentation only; no driver changes)  
**Branch:** `feature/native-ethernet`  
**Base:** `5c8b88b` (main, PhaseGuard Gate-4 merge)  
**Board:** PZ7020-SL-C / StarLite (XC7Z020)  
**Goal (later gates):** native GEM0 wire transport for existing PhaseNet — not an IP stack

```text
GEM0 + RTL8211F  →  EtherType 0x88B5  →  PN01 / PhasePacket (Ping/Pong → peer)
```

This document cites **already sealed or seller/Linux facts**. Unresolved items are listed explicitly and must not be invented.

---

## Recorded configuration

| Field | Value | Authority |
|-------|--------|-----------|
| Controller | **GEM0 / PS ENET0** (Cadence GEM); not GEM1 | [`src/board/zynq7000/gem.rs`](../../src/board/zynq7000/gem.rs); seal `run_20260730_155025` |
| GEM base | `0xE000B000` | [`src/board/zynq7000/memory_map.rs`](../../src/board/zynq7000/memory_map.rs) (`GEM0_BASE`) |
| PHY family | **RTL8211F / RTL8211FD** | Seller datasheet inventory; COM12 PHY ID match |
| PHY ID1 / ID2 | `0x001C` / `0xC916` | `fpga/zynq/evidence/kairos_zynq_boot/run_20260730_155025/` (`HONEST_RECLASS.md`) |
| PHY MDIO address | **0** (MDIO scan; not hard-coded in driver) | Same seal + `gem::scan_phy()` |
| Interface | **RGMII** / U-Boot `rgmii-id` | Seller Linux COM12: `fpga/zynq/evidence/seller_boot_smoke/com12_capture_20260729_125652.txt` |
| RGMII pins | **MIO 16–27** | [`fpga/zynq/board/pz7020-sl-c/SELLER_PACKAGE_INVENTORY.md`](../../fpga/zynq/board/pz7020-sl-c/SELLER_PACKAGE_INVENTORY.md); [`src/board/zynq7000/README.md`](../../src/board/zynq7000/README.md) |
| MDIO pins | **MDC MIO 52, MDIO MIO 53** | Seller inventory; [`OPEN_SOURCE_FINDINGS.md`](../../fpga/zynq/board/pz7020-sl-c/OPEN_SOURCE_FINDINGS.md) (`PS_GPHY_MDC/MDIO`) |
| PhaseNet EtherType | **`0x88B5`** | `gem.rs` `GP_ETHERTYPE`; V5/V6 releases; RISC-V `PHASELINK_ETHERTYPE` |
| V6 wire format | **PN01** Ping/Pong (kinds match `PhasePacketKind`) | [`fpga/zynq/release/PZ7020_GP_OS_V6/README.md`](../../fpga/zynq/release/PZ7020_GP_OS_V6/README.md); `GOLDEN_PHASE_ZYNQ_STACK.md` |
| Hosted oracle | **KPHN** link magic + `PhasePacket` (`KAIROS_PHASE_PACKET_V1`) | [`src/kernel/phasenet/link.rs`](../../src/kernel/phasenet/link.rs), [`packet.rs`](../../src/kernel/phasenet/packet.rs) |

### Packet kinds (hosted / V6 alignment — do not renumber)

| Kind | Value |
|------|------:|
| StateUpdate | 1 |
| Stream | 2 |
| Ping | 3 |
| Pong | 4 |

### Linux / seller IRQ reference (not bare-metal wiring)

Seller Linux reports Cadence GEM at `e000b000` as **irq 27** (`macb … irq 27`). This is a **Linux/seller reference only**. Bare-metal GIC SPI mapping for GEM remains **unresolved** (mainline GEM ladder is polled MMIO today).

---

## Existing mainline ladder status

Shell path: `net probe` / `net phy` / `net link` / `net loopback` / `net init` → [`src/userland/net_cmd.rs`](../../src/userland/net_cmd.rs) → `gem::*`.

| Step | Result (COM12 `run_20260730_155025`) |
|------|--------------------------------------|
| 1 Registers | PHYSICALLY VERIFIED |
| 2–3 MDIO / PHY ID | PHYSICALLY VERIFIED (RTL8211F @ addr 0) |
| 4 Link | **PARTIAL** — DOWN (no partner/cable) |
| 5 MAC local loopback `0x88B5` | PHYSICALLY VERIFIED (`rx_len=68`) — **not** wire Ethernet |
| 6 External TX/RX | **UNSUPPORTED** |

Seal index: [`docs/PZ7020_FEATURE_SEAL_20260730.md`](../PZ7020_FEATURE_SEAL_20260730.md).

### V6 PhaseNet-lite (preserve; do not overwrite)

- Evidence: `fpga/zynq/evidence/gp_os_v6/` — `GOLDEN PHASE OS PHASENET PASS`
- Transport: PhaseNet-lite over **GEM MAC loopback** (`PN01`, ethertype `0x88B5`)
- Known limitation: **MAC re-init between Ping and Pong** on a **1-BD** ring — not acceptable as final wire transport (see post-ETH-0 PN-WIRE-1 multi-BD)

### Hosted PhaseNet today

- [`src/kernel/phasenet/`](../../src/kernel/phasenet/) — modes Off / Loopback / Sim under `std`
- Bare-metal shell: `phasenet: hosted-only` (no GEM bind yet)

---

## Unresolved (ETH-0 — must not invent)

1. **Bare-metal GIC interrupt wiring** for GEM0 (Linux irq 27 is not a bare-metal proof)
2. **PHY reset** GPIO / polarity / sequence
3. **Reference-clock** source and configuration for RGMII
4. **External-wire DMA / cache** clean-invalidate behavior (not proven on cable; MAC loopback ≠ wire)
5. **Cable-negotiated speed and duplex** (link was DOWN in sealed GEM run) — decode added in ETH-1 software; physical COM12 PASS still pending (see [`ETH1_PHY_LINK_EVIDENCE.md`](ETH1_PHY_LINK_EVIDENCE.md))

---

## Stale documentation flags (fix after ETH-1 evidence)

These still claim GEM is absent / unsupported and conflict with seal `run_20260730_155025`:

- [`docs/PZ7020_FEATURE_MATRIX.md`](../PZ7020_FEATURE_MATRIX.md) — GEM row **UNSUPPORTED**
- [`README_PZ7020.md`](../../README_PZ7020.md) — GEM listed among unproven / remaining items

Do **not** rewrite the whole matrix in this ETH-0 commit.

---

## Out of scope for ETH-0

No driver changes. No ARP / IPv4 / ICMP / UDP / TCP / DHCP / SSH / web / remote shell.

PL second Ethernet (AXI / LiteX RGMII) is a separate path — not PS GEM0.

---

## Post-ETH-0 development order (not this commit)

```text
ETH-0 hardware discovery          ← this document (done)
→ ETH-1 PHY/MDIO cable-link evidence  ← PASS
→ PN-WIRE-1 external raw 0x88B5 TX/RX (multi-BD; no GEM reinit between req/reply)
     see PN_WIRE1_EXTERNAL_RAW_TXRX.md — software READY, physical pending
→ PN-WIRE-2 bare-metal PhaseNet codec (existing format)
→ PN-WIRE-3 bind PhaseNetController to GEM
→ PN-WIRE-4 bare-metal shell
→ PN-WIRE-5 external Ping/Pong peer seal
→ PN-WIRE-6 semantics + host-oracle parity + malformed safety
```

PhaseGuard remains feature-gated and read-only; do not run heavy analysis per Ethernet byte. Leave `observed_squeeze = NaN` research boundary unchanged.

---

## ETH-0 acceptance

- [x] Known hardware fields recorded with citations
- [x] Unresolved list explicit
- [x] V6 MAC-loopback seal preserved as historical
- [x] No IP-stack scaffolding
- [x] No driver / shell code changes in this gate
