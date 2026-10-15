# PN-WIRE-1 L2 Handshake Diagnostic

**Date:** 2026-08-01  
**Status:** Diagnostic only — does **not** claim PhaseNet or PN-WIRE-1 PASS.

## Purpose

Reuse the existing EtherType `0x88B5` REQ/REP path (`net wire ping` + host peer) as a one-shot check that a REQ is visible on the PC NIC. A handshake cannot invent L2 TX if GEM→PHY→wire is broken.

## Runner

```text
python kairos/scripts/pn_wire1_handshake_run.py --iface Ethernet --port COM12
```

Evidence: `kairos/fpga/zynq/evidence/kairos_zynq_boot/pn_wire1_handshake_<timestamp>/`

Verdict fields: `PEER_HEARD_REQ`, `PEER_SENT_REP`, `BOARD_PING`, `PC_BOARD_MAC_COUNT`.

## Run result (2026-08-01)

Evidence: `kairos/fpga/zynq/evidence/kairos_zynq_boot/pn_wire1_handshake_20260801_224438/`

| Field | Value |
|-------|-------|
| `PEER_HEARD_REQ` | **no** |
| `PEER_SENT_REP` | **no** |
| `BOARD_PING` | **FAIL** |
| `PC_BOARD_MAC_COUNT` | **0** |
| `PC_ETHERTYPE_88B5_COUNT` | **0** |

Rejects “handshake fixes TX.” REQ never appeared on the I225.

## Hard stop on FAIL

Recorded: **FAIL** (`PEER_HEARD_REQ=no`).

- **Stop** further GEM/handshake/PhaseNet software changes for this fault.
- **Next step only:** U15 scope A/B/C — see [`PN_WIRE1_U15_SCOPE.md`](PN_WIRE1_U15_SCOPE.md)

```text
python kairos/scripts/pn_wire1_u15_scope_run.py --port COM12
```

Do not add retries, new handshake states, PHY soft-reset, or RGMII delay experiments until A/B/C is recorded.
