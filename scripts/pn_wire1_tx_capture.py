#!/usr/bin/env python3
"""Unfiltered capture on the physical PC NIC for PN-WIRE-1 TX evidence.

Use while the board runs:  net wire ping debug

  python scripts/pn_wire1_tx_capture.py --iface Ethernet --seconds 20

Does NOT filter to 0x88B5 first — prints every frame, highlighting board MAC
02:00:00:47:01:50 and EtherType 0x88B5. Resolves to physical I225 Npcap device
(not Wi-Fi / Hyper-V / VMware).
"""

from __future__ import annotations

import argparse
import importlib.util
import sys
import time

BOARD = bytes([0x02, 0x00, 0x00, 0x47, 0x01, 0x50])


def load_resolve():
    path = __file__.replace("pn_wire1_tx_capture.py", "pn_wire1a_l2_bridge.py")
    spec = importlib.util.spec_from_file_location("bridge", path)
    mod = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(mod)
    return mod.resolve_iface


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--iface", default="Ethernet")
    ap.add_argument("--seconds", type=float, default=20.0)
    args = ap.parse_args()

    try:
        from scapy.all import sniff  # type: ignore
    except ImportError:
        print("pip install scapy", file=sys.stderr)
        return 1

    resolve = load_resolve()
    iface = resolve(args.iface)
    print(f"[txcap] iface request={args.iface!r} resolved={iface!r}")
    print("[txcap] unfiltered capture — look for src=020000470150 ethertype=0x88b5")
    print("[txcap] on board: net wire init ; net wire ping debug")
    t0 = time.time()
    hits_board = 0
    hits_88b5 = 0
    total = 0

    def on_pkt(pkt) -> None:
        nonlocal hits_board, hits_88b5, total
        b = bytes(pkt)
        if len(b) < 14:
            return
        total += 1
        et = (b[12] << 8) | b[13]
        src = b[6:12]
        dst = b[0:6]
        mark = ""
        if src == BOARD or dst == BOARD:
            hits_board += 1
            mark = " <<BOARD"
        if et == 0x88B5:
            hits_88b5 += 1
            mark += " <<88B5"
        if mark or total <= 8:
            print(
                f"t+{time.time()-t0:5.2f}s len={len(b):4d} et=0x{et:04X} "
                f"dst={dst.hex()} src={src.hex()}{mark}"
            )

    sniff(iface=iface, store=False, promisc=True, timeout=args.seconds, prn=on_pkt)
    print(f"[txcap] done total={total} board_mac={hits_board} ethertype_88b5={hits_88b5}")
    if hits_board == 0 and hits_88b5 == 0:
        print("[txcap] NO board frame seen — TX not on this NIC/L2 (or not leaving PHY)")
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
