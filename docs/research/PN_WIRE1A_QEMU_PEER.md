# PN-WIRE-1A — Physical PZ7020 ↔ QEMU Peer Proof

**Classification:** Physical PZ7020-to-QEMU PhaseNet peer proof (development)  
**Not:** a two-board PhaseNet seal (that is **PN-WIRE-1B**)

```text
PZ7020 GEM0 + RTL8211F
  → real cable
  → PC NIC
  → pn_wire1a_l2_bridge.py  (Npcap ↔ QEMU socket)
  → QEMU virtio-net
  → Golden Phase `net peer`  (PNW1 REQ → controlled REP)
```

## Milestone split

| Gate | Path | Role |
|------|------|------|
| **PN-WIRE-1A** | PZ7020 ↔ QEMU over physical cable | Strong development proof |
| **PN-WIRE-1B** | PZ7020 ↔ PZ7020 over physical cable | Final hardware peer seal |

Do not wait for a second board to continue 1A.  
Do not label 1A results as a two-board seal.

---

## Software added

| Piece | Path |
|-------|------|
| Shared frame contract | `src/protocol/pnw1_wire.rs` |
| QEMU peer | `src/boot/arch/riscv64/pnw1_peer.rs` + `net peer [n]` |
| QEMU boot | `scripts/boot-qemu-pn-wire1a.ps1` (`bm-embedded`, socket listen) |
| Host L2 bridge | `scripts/pn_wire1a_l2_bridge.py` |
| Board wire path | existing `net wire *` (GEM multi-BD) |

Frame bytes match board GEM PNW1 (EtherType `0x88B5`, magic, kind, seq, cookie, pad).

---

## Topology

```text
PZ7020 GEM0 ──┐
              ├── same switch / same L2
PC Ethernet ──┘── pn_wire1a_l2_bridge.py ── QEMU socket ── net peer
```

Direct PC↔board also works. Both-to-switch is preferred.

**Windows note:** Hyper-V may expose `vEthernet (WSL-Bridge)` with the same MAC as Intel I225. The bridge script resolves `--iface Ethernet` to the physical I225 Npcap device (`\Device\NPF_{…}`), not Wi-Fi.

## Operator procedure

1. Flash / boot PZ7020 PN-WIRE image; GEM0 + PC Ethernet on the **same switch** (`net link` UP).
2. Terminal A — QEMU peer:
   ```powershell
   powershell -ExecutionPolicy Bypass -File scripts\boot-qemu-pn-wire1a.ps1
   ```
   At `gpos>`:
   ```text
   net init
   net peer 16
   ```
3. Terminal B — host bridge (needs scapy + Npcap; admin if required):
   ```powershell
   pip install scapy
   python scripts\pn_wire1a_l2_bridge.py --iface Ethernet --qemu 127.0.0.1:1234 --filter-88b5
   ```
4. Terminal C — COM12:
   ```text
   net wire init
   net wire ping
   ```
5. Expect board: `gem wire ping: PASS ... (no GEM reinit)`  
   Expect QEMU: `net peer: OK replied=…`  
   Expect bridge logs: nic↔qemu `et=0x88B5`

Linux alternative: bridge physical NIC to a TAP and use QEMU `-netdev tap` (same guest `net peer` command).

---

## Acceptance (1A)

| Check | Status |
|-------|--------|
| Shared PNW1 contract unit test | host `cargo test pnw1` |
| QEMU `net peer` builds (`bm-embedded`) | pending local run |
| Bridge forwards 0x88B5 both ways | pending |
| Board `net wire ping` PASS via QEMU REP | pending |
| `init_count` unchanged / no LOOP | pending |

---

## Explicitly not claimed

- Two-board PhaseNet seal (PN-WIRE-1B)
- PhaseNet codec bind (PN-WIRE-2+)
- IP stack
