# PN-WIRE-1 — External Raw EtherType `0x88B5` TX/RX

**Branch:** `feature/native-ethernet`  
**Gate:** PN-WIRE-1  
**Depends on:** ETH-0, ETH-1 PASS  
**Scope:** GEM0 + RTL8211F multi-BD raw L2 only  

```text
GEM0 + RTL8211F → cable → raw 0x88B5 TX → peer RX → controlled REP → board RX
```

## Verdict (current)

| Item | Status |
|------|--------|
| Multi-BD wire path software | **READY** (`gem::wire_*`, `net wire …`) |
| Shared PNW1 contract | **READY** (`protocol/pnw1_wire.rs`) |
| PN-WIRE-1A QEMU peer path | **READY** — see [`PN_WIRE1A_QEMU_PEER.md`](PN_WIRE1A_QEMU_PEER.md) |
| Host Python echo peer | **READY** (`pn_wire1_host_peer.py`) — alternate, not PhaseNet seal |
| `BOOT.BIN` packaged | `2BCE72E4…` — `BOOT_PZ7020_PN_WIRE1/` (includes `net wire ping debug`) |
| Flash to microSD | **DONE** prior image; rebuild/flash if running post-unify tree |
| PN-WIRE-1A physical evidence | **NOT PASS** — need NIC up + QEMU peer + bridge + `net wire ping` |
| PN-WIRE-1B two-board seal | **NOT STARTED** |
| PhaseNet codec / shell bind | **NOT STARTED** (PN-WIRE-2+) |

### Milestone honesty

| Gate | Meaning |
|------|---------|
| **PN-WIRE-1A** | PZ7020 ↔ QEMU over physical cable — development proof |
| **PN-WIRE-1B** | PZ7020 ↔ PZ7020 over physical cable — final hardware peer seal |

Call 1A: **Physical PZ7020-to-QEMU PhaseNet peer proof**. Never “two-board seal”.

Do **not** claim PASS from `net loopback` / MAC loopback.  
Do **not** label a host-peer run as a two-board PhaseNet seal.

Suggested commit message **only after physical PASS**:

> Zynq GEM: finish external raw 0x88B5 TX/RX

---

## Boundaries (enforced)

- Multi-BD rings (`WIRE_RING=4`); wrap on last BD
- No GEM reinitialization between REQ TX and REP RX (`init_count` must be unchanged; `LOOP` bit must stay clear)
- No PhaseNet codec, no `phasenet` shell changes
- No ARP / IPv4 / UDP / TCP / DHCP / SSH
- No second PL Ethernet path

---

## Frame format (raw PNW1 — not PhaseNet)

| Offset | Field |
|--------|--------|
| 0..5 | dst MAC (`FF:FF:FF:FF:FF:FF` for REQ) |
| 6..11 | src MAC board `02:00:00:47:01:50` |
| 12..13 | EtherType `0x88B5` |
| 14..17 | magic `PNW1` |
| 18 | kind `0x01` REQ / `0x02` REP |
| 19 | seq |
| 20..27 | cookie `C0 FF EE 11 22 33 44 55` |
| 28..63 | pad `A0,A1,…` |

Board accepts REP only when payload matches controlled template (exact bytes).

---

## Shell (board)

```text
net link
net wire init
net wire status
net wire ping          # TX REQ + RX REP, no GEM reinit
net wire wrap          # RING+1 TX submits; proves TX wrap
net wire recover       # link re-sync without BD reset
```

`net loopback` remains MAC-local and **invalidates** the wire path (must `net wire init` again).

---

## Operator physical procedure

1. Boot ETH-1-capable / PN-WIRE-1 image; confirm `gpos>` and `net link` UP on **GEM0** RJ45.
2. On host (same L2 segment / direct cable):
   ```bash
   sudo python3 scripts/pn_wire1_host_peer.py --iface <NIC>
   ```
   Windows: Npcap + `pip install scapy`, then `--backend scapy`.
3. On COM12:
   ```text
   net wire init
   net wire ping
   net wire wrap
   ```
4. Disconnect/reconnect cable → `net wire recover` → `net wire ping` again.
5. Capture UART transcript under `fpga/zynq/evidence/kairos_zynq_boot/pn_wire1_*`.
6. Update this document’s acceptance table to PASS only with that evidence.

Prefer two PZ7020 boards when available; host peer is acceptable for this gate only.

---

## Acceptance checklist

| Check | Status |
|-------|--------|
| Link UP | pending |
| TX frame leaves GEM0 | pending |
| Peer receives EtherType `0x88B5` | pending |
| Payload + length match | pending |
| Peer returns controlled REP | pending |
| Board RX exact REP bytes | pending |
| RX/TX descriptors return ownership | pending |
| Ring wrap works (`net wire wrap`) | pending |
| Disconnect/reconnect recovers | pending |
| No GEM reset between TX and RX reply | pending |

---

## PHY PCS loopback isolation (next proof)

RGMII delay experiment: **FAIL / reverted** (PC still 0 frames). See [`PN_WIRE1_PHY_LOOPBACK.md`](PN_WIRE1_PHY_LOOPBACK.md).

```text
gpos> net wire init
gpos> net wire phyloop
gpos> net wire txburst 32   # optional scope aid
```

## TX-leaves-GEM0 diagnostic

External reply is blocked until a frame is **visible**. First prove GEM TX:

```text
gpos> net wire init
gpos> net wire ping debug
```

Reports: exact broadcast `0x88B5` frame, BD before/after (USED/LAST/WRAP/len),
`NWCTRL`/`NWCFG`/`TXQBASE`/`TXSR`/`ISR`, ownership return, TX complete/error decode.

| Observation | Meaning |
|-------------|---------|
| BD ownership returns + TXSR.COMPLETE, no error | GEM believes it transmitted |
| BD never returns | DMA / descriptor setup |
| Ownership + TXSR error | MAC/DMA/clock/config |
| GEM OK + PC sees nothing | PHY/RGMII / L2 path / wrong capture iface |

PC unfiltered capture (physical I225, not Wi-Fi/Hyper-V):

```powershell
Get-NetAdapter
python scripts\pn_wire1_tx_capture.py --iface Ethernet --seconds 20
```

Prefer direct GEM0 ↔ PC Ethernet while debugging TX emission.

## Explicitly not started

- PN-WIRE-2 bare-metal PhaseNet codec
- PN-WIRE-3 bind PhaseNetController to GEM
- PN-WIRE-4 / 5 / 6
- IP stack
