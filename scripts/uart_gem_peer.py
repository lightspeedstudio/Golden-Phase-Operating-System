#!/usr/bin/env python3
"""Host peer for the PZ7020 UART-GEM PhaseNet bridge (EtherType 0x88B5).

Validates KPHN RawStream frames (magic, length, SHA-256 frame hash, sequence)
and replies with an ACK payload. Not a two-board PhaseNet seal.

  python scripts/uart_gem_peer.py --iface Ethernet --timeout 60
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import struct
import sys
import time
from pathlib import Path

MAGIC = b"KPHN"
TAG = b"KAIROS_PHASE_LINK_FRAME_V1"
ETHERTYPE = 0x88B5
KIND_RAW = 2
MIN_BODY = 4 + 1 + 32 * 4 + 4 + 8 + 8 + 8 + 1 + 8 + 1 + 4 + 32 + 32 + 32


def resolve_iface(name: str) -> str:
    path = Path(__file__).with_name("pn_wire1a_l2_bridge.py")
    spec = importlib.util.spec_from_file_location("pn_wire1a_l2_bridge", path)
    if spec is None or spec.loader is None:
        return name
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod.resolve_iface(name)


def frame_hash(body: bytes) -> bytes:
    h = hashlib.sha256()
    h.update(TAG)
    h.update(body)
    return h.digest()


def decode(payload: bytes) -> dict | None:
    if len(payload) < MIN_BODY or payload[:4] != MAGIC:
        return None
    off = 4
    ver = payload[off]
    off += 1
    if ver != 1:
        return None
    src = payload[off : off + 32]
    off += 32
    dst = payload[off : off + 32]
    off += 32
    boot = payload[off : off + 32]
    off += 32
    eq = payload[off : off + 32]
    off += 32
    chan = struct.unpack_from("<I", payload, off)[0]
    off += 4
    seq = struct.unpack_from("<Q", payload, off)[0]
    off += 8
    proper = struct.unpack_from("<Q", payload, off)[0]
    off += 8
    kairos = struct.unpack_from("<Q", payload, off)[0]
    off += 8
    sector = payload[off]
    off += 1
    x = struct.unpack_from("<d", payload, off)[0]
    off += 8
    kind = payload[off]
    off += 1
    plen = struct.unpack_from("<I", payload, off)[0]
    off += 4
    if off + plen + 96 != len(payload):
        return None
    data = payload[off : off + plen]
    off += plen
    proof = payload[off : off + 32]
    off += 32
    prev = payload[off : off + 32]
    off += 32
    fh = payload[off : off + 32]
    body = payload[:-32]
    if frame_hash(body) != fh:
        return None
    return {
        "src": src,
        "dst": dst,
        "boot": boot,
        "eq": eq,
        "chan": chan,
        "seq": seq,
        "proper": proper,
        "kairos": kairos,
        "sector": sector,
        "x": x,
        "kind": kind,
        "data": data,
        "proof": proof,
        "prev": prev,
        "hash": fh,
        "raw": payload,
    }


def encode_reply(req: dict, text: bytes, seq: int, prev: bytes) -> bytes:
    body = bytearray()
    body += MAGIC
    body += bytes([1])
    body += req["dst"]
    body += req["src"]
    body += req["boot"]
    body += req["eq"]
    body += struct.pack("<I", req["chan"])
    body += struct.pack("<Q", seq)
    body += struct.pack("<Q", seq)
    body += struct.pack("<Q", 0)
    body += bytes([0])
    body += struct.pack("<d", 0.0)
    body += bytes([KIND_RAW])
    body += struct.pack("<I", len(text))
    body += text
    body += bytes(32)
    body += prev
    fh = frame_hash(bytes(body))
    return bytes(body) + fh


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--iface", default="Ethernet")
    ap.add_argument("--timeout", type=float, default=60.0)
    ap.add_argument("--count", type=int, default=1)
    ap.add_argument("--ready", default="", help="touch this path once sniff is armed")
    args = ap.parse_args()
    iface = resolve_iface(args.iface)
    from scapy.all import Ether, sendp, sniff  # type: ignore

    print(f"[peer] uart-gem iface={iface} ethertype=0x{ETHERTYPE:04X}", flush=True)
    if args.ready:
        Path(args.ready).write_text("ready\n", encoding="utf-8")
    seen = 0
    last_seq = 0
    last_hash = bytes(32)
    deadline = time.time() + args.timeout

    def handle(pkt) -> None:
        nonlocal seen, last_seq, last_hash
        raw = bytes(pkt)
        if len(raw) < 14:
            return
        et = struct.unpack("!H", raw[12:14])[0]
        if et != ETHERTYPE:
            return
        dec = decode(raw[14:])
        if dec is None:
            print("[peer] refuse malformed", flush=True)
            return
        if dec["seq"] <= last_seq:
            print(f"[peer] refuse replay seq={dec['seq']}", flush=True)
            return
        last_seq = dec["seq"]
        text = dec["data"]
        print(f"[peer] rx seq={dec['seq']} payload={text!r}", flush=True)
        ack = b"ack:" + text
        body = encode_reply(dec, ack, dec["seq"], dec["hash"])
        board_mac = raw[6:12]
        host_mac = bytes.fromhex(str(pkt.src).replace(":", ""))
        eth = board_mac + host_mac + struct.pack("!H", ETHERTYPE) + body
        sendp(Ether(eth), iface=iface, verbose=False)
        print(f"[peer] tx ack seq={dec['seq']}", flush=True)
        seen += 1

    while seen < args.count and time.time() < deadline:
        sniff(
            iface=iface,
            store=False,
            timeout=min(2.0, max(0.1, deadline - time.time())),
            prn=handle,
            stop_filter=lambda p: seen >= args.count,
        )
    print(f"[peer] done echoed={seen}", flush=True)
    return 0 if seen > 0 else 2


if __name__ == "__main__":
    sys.exit(main())
