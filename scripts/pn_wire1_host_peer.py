#!/usr/bin/env python3
"""PN-WIRE-1 raw Ethernet host peer (NOT a two-board PhaseNet seal).

Listens for EtherType 0x88B5 PNW1 REQ frames from the PZ7020 board MAC
02:00:00:47:01:50, verifies exact payload layout, and returns a controlled
PNW1 REP (kind 0x01 -> 0x02, same seq/cookie/pad).

Linux (preferred):
  sudo python3 scripts/pn_wire1_host_peer.py --iface eth0

Windows: requires Npcap + scapy:
  pip install scapy
  python scripts/pn_wire1_host_peer.py --iface \"Ethernet\" --backend scapy

This peer proves raw L2 TX/RX only. Do not label results as PhaseNet PASS.
"""

from __future__ import annotations

import argparse
import importlib.util
import socket
import struct
import sys
import time
from pathlib import Path


def resolve_iface(name_or_mac: str) -> str:
    """Same Windows Npcap resolver as pn_wire1a_l2_bridge / tx_capture."""
    path = Path(__file__).with_name("pn_wire1a_l2_bridge.py")
    spec = importlib.util.spec_from_file_location("pn_wire1a_l2_bridge", path)
    if spec is None or spec.loader is None:
        return name_or_mac
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod.resolve_iface(name_or_mac)

ETH_P_ALL = 0x0003
ETHERTYPE = 0x88B5
BOARD_MAC = bytes([0x02, 0x00, 0x00, 0x47, 0x01, 0x50])
MAGIC = b"PNW1"
KIND_REQ = 0x01
KIND_REP = 0x02
COOKIE = bytes([0xC0, 0xFF, 0xEE, 0x11, 0x22, 0x33, 0x44, 0x55])
FRAME_LEN = 64


def mac_str(b: bytes) -> str:
    return ":".join(f"{x:02x}" for x in b)


def build_expected_req_payload(seq: int) -> bytes:
    pad = bytes((0xA0 + i) & 0xFF for i in range(FRAME_LEN - 28))
    return MAGIC + bytes([KIND_REQ, seq & 0xFF]) + COOKIE + pad


def build_rep_frame(src_mac: bytes, seq: int) -> bytes:
    pad = bytes((0xA0 + i) & 0xFF for i in range(FRAME_LEN - 28))
    payload = MAGIC + bytes([KIND_REP, seq & 0xFF]) + COOKIE + pad
    dst = BOARD_MAC
    return dst + src_mac + struct.pack("!H", ETHERTYPE) + payload


def parse_pnw1(frame: bytes) -> tuple[bytes, bytes, int, bytes] | None:
    if len(frame) < FRAME_LEN:
        return None
    dst, src = frame[0:6], frame[6:12]
    et = struct.unpack("!H", frame[12:14])[0]
    if et != ETHERTYPE:
        return None
    payload = frame[14:FRAME_LEN]
    if payload[0:4] != MAGIC:
        return None
    return dst, src, payload[4], payload


def run_afpacket(iface: str, count: int, timeout: float) -> int:
    sock = socket.socket(socket.AF_PACKET, socket.SOCK_RAW, socket.htons(ETH_P_ALL))
    sock.bind((iface, 0))
    sock.settimeout(1.0)
    src_mac = sock.getsockname()[4]
    if len(src_mac) != 6:
        # Some kernels return longer sockaddr; take first 6.
        src_mac = bytes(src_mac[:6])
    print(f"[peer] AF_PACKET iface={iface} mac={mac_str(src_mac)} ethertype=0x{ETHERTYPE:04X}")
    print("[peer] waiting for PNW1 REQ (host peer — not PhaseNet seal)")
    seen = 0
    deadline = time.time() + timeout
    while seen < count and time.time() < deadline:
        try:
            frame = sock.recv(2048)
        except socket.timeout:
            continue
        parsed = parse_pnw1(frame)
        if not parsed:
            continue
        dst, src, kind, payload = parsed
        if src != BOARD_MAC or kind != KIND_REQ:
            continue
        seq = payload[5]
        expect = build_expected_req_payload(seq)
        if payload != expect:
            print(f"[peer] REQ mismatch seq={seq} len={len(payload)}")
            print(f"  got:  {payload.hex()}")
            print(f"  want: {expect.hex()}")
            continue
        print(f"[peer] REQ OK seq={seq} from {mac_str(src)} dst={mac_str(dst)} len={FRAME_LEN}")
        rep = build_rep_frame(src_mac, seq)
        sock.send(rep)
        print(f"[peer] REP sent seq={seq} len={len(rep)} (controlled PNW1 reply)")
        seen += 1
    sock.close()
    print(f"[peer] done echoed={seen}")
    return 0 if seen > 0 else 2


def run_scapy(iface: str, count: int, timeout: float) -> int:
    try:
        from scapy.all import Ether, sendp, sniff  # type: ignore
    except ImportError:
        print("scapy not installed; pip install scapy (and Npcap on Windows)", file=sys.stderr)
        return 1

    print(f"[peer] scapy iface={iface} ethertype=0x{ETHERTYPE:04X}")
    print("[peer] waiting for PNW1 REQ (host peer — not PhaseNet seal)")
    seen = 0

    def handle(pkt) -> None:
        nonlocal seen
        if seen >= count:
            return
        raw = bytes(pkt)
        parsed = parse_pnw1(raw)
        if not parsed:
            return
        dst, src, kind, payload = parsed
        if src != BOARD_MAC or kind != KIND_REQ:
            return
        seq = payload[5]
        expect = build_expected_req_payload(seq)
        if payload != expect:
            print(f"[peer] REQ mismatch seq={seq}")
            return
        print(f"[peer] REQ OK seq={seq} from {mac_str(src)}")
        rep_payload = MAGIC + bytes([KIND_REP, seq]) + COOKIE
        rep_payload += bytes((0xA0 + i) & 0xFF for i in range(FRAME_LEN - 28))
        sendp(
            Ether(dst=mac_str(BOARD_MAC), type=ETHERTYPE) / rep_payload,
            iface=iface,
            verbose=False,
        )
        print(f"[peer] REP sent seq={seq}")
        seen += 1

    sniff(
        iface=iface,
        filter=f"ether proto 0x{ETHERTYPE:04x}",
        prn=handle,
        timeout=timeout,
        store=False,
        stop_filter=lambda _: seen >= count,
    )
    print(f"[peer] done echoed={seen}")
    return 0 if seen > 0 else 2


def main() -> int:
    ap = argparse.ArgumentParser(description="PN-WIRE-1 raw 0x88B5 host peer")
    ap.add_argument("--iface", required=True, help="host NIC name (e.g. eth0, Ethernet)")
    ap.add_argument("--count", type=int, default=8, help="number of REQ frames to echo")
    ap.add_argument("--timeout", type=float, default=120.0, help="seconds to listen")
    ap.add_argument(
        "--backend",
        choices=("auto", "afpacket", "scapy"),
        default="auto",
        help="packet backend",
    )
    args = ap.parse_args()

    iface = args.iface
    if sys.platform.startswith("win"):
        resolved = resolve_iface(args.iface)
        if resolved != args.iface:
            print(f"[peer] resolved iface {args.iface!r} -> {resolved!r}")
        iface = resolved

    backend = args.backend
    if backend == "auto":
        backend = "afpacket" if sys.platform.startswith("linux") else "scapy"

    if backend == "afpacket":
        if not hasattr(socket, "AF_PACKET"):
            print("AF_PACKET unavailable; use --backend scapy", file=sys.stderr)
            return 1
        return run_afpacket(iface, args.count, args.timeout)
    return run_scapy(iface, args.count, args.timeout)


if __name__ == "__main__":
    sys.exit(main())
