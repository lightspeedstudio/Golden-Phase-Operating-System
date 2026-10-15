#!/usr/bin/env python3
"""Host Npcap sniff for A16/A28: count board MAC + EtherType 0x88B5.

Writes a HOST_PARSER_EVENT text sidecar. Does not talk to COM12.
Does not write into a UART file. Exit 0 only if seq_run >= 32 and both counts >= 32.
"""

from __future__ import annotations

import argparse
import sys
import time
from pathlib import Path

BOARD = bytes([0x02, 0x00, 0x00, 0x47, 0x01, 0x50])


def load_resolve():
    import importlib.util

    path = Path(__file__).with_name("pn_wire1a_l2_bridge.py")
    spec = importlib.util.spec_from_file_location("bridge", path)
    mod = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(mod)
    return mod.resolve_iface


def longest_run(seqs: list[int]) -> int:
    sset = set(seqs)
    best = 0
    for x in sset:
        if (x - 1) not in sset:
            n = 1
            while (x + n) in sset:
                n += 1
            if n > best:
                best = n
    return best


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--iface", default="Ethernet")
    ap.add_argument("--seconds", type=float, default=20.0)
    ap.add_argument("--out", required=True)
    ap.add_argument("--ready", default="")
    args = ap.parse_args()
    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)

    try:
        from scapy.all import AsyncSniffer  # type: ignore
    except ImportError:
        out.write_text("FAIL: pip install scapy\n", encoding="utf-8")
        print("FAIL: pip install scapy", file=sys.stderr)
        return 1

    resolve = load_resolve()
    iface = resolve(args.iface)
    lines = [
        f"[txcap] type=HOST_PARSER_EVENT",
        f"[txcap] iface request={args.iface!r} resolved={iface!r}",
        "[txcap] unfiltered capture — src=020000470150 ethertype=0x88b5",
        "[txcap] on board: net wire init ; net wire txburst 32",
    ]
    hits_board = 0
    hits_88b5 = 0
    total = 0
    seqs: list[int] = []

    def on_pkt(pkt) -> None:
        nonlocal hits_board, hits_88b5, total
        b = bytes(pkt)
        if len(b) < 14:
            return
        total += 1
        et = (b[12] << 8) | b[13]
        src = b[6:12]
        dst = b[0:6]
        board = src == BOARD or dst == BOARD
        if board:
            hits_board += 1
        if et == 0x88B5:
            hits_88b5 += 1
        seq = b[19] if len(b) > 19 else -1
        if et == 0x88B5 and src == BOARD:
            seqs.append(seq)

    try:
        sniffer = AsyncSniffer(
            iface=iface,
            store=True,
            promisc=True,
            timeout=args.seconds,
            prn=on_pkt,
        )
        sniffer.start()
        if args.ready:
            Path(args.ready).write_text("listening\n", encoding="utf-8")
        sniffer.join()
    except Exception as e:
        lines.append(f"[txcap] sniff_failed err={e}")
        out.write_text("\n".join(lines) + "\n", encoding="utf-8")
        print(f"FAIL: sniff {e}", flush=True)
        return 1

    seq_run = longest_run(seqs)
    lines.append(
        f"[txcap] done total={total} board_mac={hits_board} ethertype_88b5={hits_88b5} seq_run={seq_run}"
    )
    if seq_run < 32 or hits_board < 32 or hits_88b5 < 32:
        lines.append("[txcap] NO 32/32 board 0x88B5 — TX not on this NIC/L2")
    body = "\n".join(lines) + "\n"
    out.write_text(body, encoding="utf-8")
    sys.stdout.write(body)
    sys.stdout.flush()
    if seq_run < 32 or hits_board < 32 or hits_88b5 < 32:
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
