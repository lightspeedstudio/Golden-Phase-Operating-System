#!/usr/bin/env python3
"""Send PhaseLink EtherType 0x88B5 AI_JOB / AI_ACT / AI_CANCEL frames to PZ7020.

This is not JSON-RPC. Frames are KPL1 proof-framed PhaseLink (same codec as the
board). Requires a raw Ethernet path (Npcap on Windows). If the adapter cannot
send 0x88B5, this script says so and writes the frames to disk — it does not
fall back to TCP JSON.

Usage:
  python ai_phaselink_client.py --op job [--iface NAME]
  python ai_phaselink_client.py --op act --job-id N --parent HEX64 [--iface NAME]
  python ai_phaselink_client.py --op deny --job-id N --parent HEX64 [--iface NAME]
  python ai_phaselink_client.py --op cancel --job-id N --phase 2 [--iface NAME]
  python ai_phaselink_client.py --op quarantine [--iface NAME]
"""

from __future__ import annotations

import argparse
import hashlib
import socket
import struct
import sys
from pathlib import Path

ETHERTYPE = 0x88B5
BOARD_MAC = bytes([0x02, 0x00, 0x00, 0x47, 0x01, 0x50])
MAGIC = b"KPL1"
FRAME_TAG = b"KAIROS_PHASELINK_FRAME_V1"
PAYLOAD_TAG = b"KAIROS_PAYLOAD_V1"
EQ_TAG = b"KAIROS_BAREMETAL_EQUATION_V1"
BOOT_TAG = b"KAIROS_BAREMETAL_BOOT_SESSION_V1"
NODE_TAG = b"KAIROS_PHASELINK_NODE_V1"
COMMIT_TAG = b"KAIROS_PHASELINK_PUBLIC_COMMITMENT_V1"


def tagged_hash(tag: bytes, parts: list[bytes]) -> bytes:
    h = hashlib.sha256()
    h.update(tag)
    for p in parts:
        h.update(p)
    return h.digest()


def write_str(buf: bytearray, s: str) -> None:
    b = s.encode("utf-8")
    buf.extend(struct.pack(">H", len(b)))
    buf.extend(b)


def parse_hex32(s: str) -> bytes:
    t = s.replace(":", "").replace(" ", "").strip()
    raw = bytes.fromhex(t)
    if len(raw) != 32:
        raise ValueError(f"need 32-byte hex, got {len(raw)}")
    return raw


def serialize_proof(
    object_kind: int,
    object_id: int,
    seq: int,
    prev_phase: int,
    next_phase: int,
    operation: str,
    payload_hash: bytes,
    prev_hash: bytes,
    equation_root: bytes,
) -> bytes:
    out = bytearray()
    out.append(object_kind)
    out.extend(struct.pack(">I", object_id))
    out.extend(struct.pack(">Q", seq))
    out.append(prev_phase)
    out.append(next_phase)
    write_str(out, operation)
    out.append(255)  # compose none
    out.append(0)  # topology none
    out.extend(payload_hash)
    out.extend(prev_hash)
    out.extend(equation_root)
    return bytes(out)


def build_frame(
    *,
    src: bytes,
    dst: bytes,
    equation: bytes,
    boot: bytes,
    seq: int,
    prev_hash: bytes,
    operation: str,
    object_id: str,
    prev_phase: int,
    next_phase: int,
    payload: bytes,
    corrupt: bool = False,
) -> bytes:
    payload_hash = tagged_hash(PAYLOAD_TAG, [payload])
    proof = serialize_proof(7, 6, seq, prev_phase, next_phase, operation, payload_hash, prev_hash, equation)
    commit = tagged_hash(COMMIT_TAG, [payload_hash, prev_hash, struct.pack(">Q", seq)])
    body = bytearray()
    body.extend(MAGIC)
    body.append(1)  # version
    body.append(0x08)  # proof_required
    body.extend(b"\x00\x00")  # header len placeholder
    body.extend(b"\x00\x00\x00\x00")  # total len placeholder
    body.extend(src)
    body.extend(dst)
    body.extend(equation)
    body.extend(boot)
    body.extend(struct.pack(">Q", seq))
    body.extend(struct.pack(">Q", 0))  # chronos
    body.extend(struct.pack(">Q", 0))  # kairos
    body.append(0)  # sector
    body.append(7)  # Driver
    write_str(body, object_id)
    write_str(body, operation)
    body.append(prev_phase)
    body.append(next_phase)
    body.extend(prev_hash)
    body.extend(struct.pack(">I", len(proof)))
    body.extend(proof)
    body.append(1)  # RawBinary
    body.extend(struct.pack(">I", len(payload)))
    body.extend(payload)
    body.extend(payload_hash)
    body.extend(commit)
    header_len = 12
    total_len = len(body) + 32
    body[6:8] = struct.pack(">H", header_len)
    body[8:12] = struct.pack(">I", total_len)
    frame_hash = tagged_hash(FRAME_TAG, [bytes(body)])
    if corrupt:
        frame_hash = bytes(b ^ 0xFF for b in frame_hash)
    body.extend(frame_hash)
    return bytes(body)


def wrap_eth(src_mac: bytes, dst_mac: bytes, payload: bytes) -> bytes:
    return dst_mac + src_mac + struct.pack(">H", ETHERTYPE) + payload


def send_raw(iface: str, frame: bytes) -> None:
    if sys.platform.startswith("linux"):
        s = socket.socket(socket.AF_PACKET, socket.SOCK_RAW)
        s.bind((iface, ETHERTYPE))
        s.send(frame)
        s.close()
        return
    try:
        from scapy.all import Ether, sendp  # type: ignore
    except Exception as e:
        raise RuntimeError(
            f"cannot send EtherType 0x88B5 on {sys.platform}: install Npcap+scapy "
            f"or use Linux AF_PACKET. {e}"
        ) from e
    pkt = Ether(frame)
    sendp(pkt, iface=iface, verbose=False)


def parse_mac(s: str) -> bytes:
    return bytes(int(x, 16) for x in s.split(":"))


def main() -> int:
    p = argparse.ArgumentParser(description="PhaseLink 0x88B5 AI control client")
    p.add_argument("--iface", help="ethernet interface name")
    p.add_argument("--dst", default="02:00:00:47:01:50")
    p.add_argument("--src-mac", default="02:00:00:47:01:51")
    p.add_argument(
        "--op",
        choices=("job", "act", "deny", "cancel", "quarantine"),
        default="job",
        help="frame kind: job, Phi3 observe act, denied shell.execute, cancel Phi1→Phi0, or illegal Phi2→Phi0 quarantine",
    )
    p.add_argument("--job-id", type=int, default=0)
    p.add_argument("--parent", default="", help="64-hex parent_chain_hash from UART ai show")
    p.add_argument("--tool", default="system.version")
    p.add_argument("--args", default="")
    p.add_argument("--seq", type=int, default=1)
    p.add_argument("--prev", default="", help="64-hex previous_frame_hash (zeros if first)")
    p.add_argument("--phase", type=int, default=-1, help="previous_phase override (cancel default 2)")
    p.add_argument("--out", default="", help="write ethernet frame here")
    args = p.parse_args()

    equation = tagged_hash(EQ_TAG, [b"cluster"])
    boot_ai = tagged_hash(BOOT_TAG, [b"ai"])
    boot_board = tagged_hash(BOOT_TAG, [b"board"])
    src = tagged_hash(NODE_TAG, [b"ai", equation, boot_ai])
    dst = tagged_hash(NODE_TAG, [b"board", equation, boot_board])
    prev = parse_hex32(args.prev) if args.prev else bytes(32)

    if args.op == "job":
        operation, object_id, prev_phase, next_phase = "AI_JOB", "new", 0, 1
        payload = b"external-ai"
        corrupt = False
    elif args.op == "act":
        if args.job_id <= 0 or not args.parent:
            print("act requires --job-id and --parent (64 hex from UART ai show)", file=sys.stderr)
            return 2
        operation = "AI_ACT"
        object_id = f"{args.job_id}/{args.tool}"
        prev_phase, next_phase = 2, 3
        payload = parse_hex32(args.parent) + args.args.encode("utf-8")
        corrupt = False
    elif args.op == "deny":
        if args.job_id <= 0 or not args.parent:
            print("deny requires --job-id and --parent", file=sys.stderr)
            return 2
        operation = "AI_ACT"
        object_id = f"{args.job_id}/shell.execute"
        prev_phase, next_phase = 2, 3
        payload = parse_hex32(args.parent) + b"id"
        corrupt = False
    elif args.op == "cancel":
        if args.job_id <= 0:
            print("cancel requires --job-id", file=sys.stderr)
            return 2
        operation = "AI_CANCEL"
        object_id = str(args.job_id)
        prev_phase = 1 if args.phase < 0 else args.phase
        next_phase = 0
        payload = b""
        corrupt = False
    else:
        # Legal decode, illegal hop → accept_or_quarantine quarantines (does not execute).
        operation, object_id, prev_phase, next_phase = "AI_ACT", "1/system.version", 2, 0
        payload = bytes(32)
        corrupt = False

    frame = build_frame(
        src=src,
        dst=dst,
        equation=equation,
        boot=boot_ai,
        seq=args.seq,
        prev_hash=prev,
        operation=operation,
        object_id=object_id,
        prev_phase=prev_phase,
        next_phase=next_phase,
        payload=payload,
        corrupt=corrupt,
    )
    eth = wrap_eth(parse_mac(args.src_mac), parse_mac(args.dst), frame)
    out_path = Path(args.out) if args.out else Path(f"ai_phaselink_{args.op}.bin")
    out_path.write_bytes(eth)
    print(
        f"wrote {out_path} ({len(eth)} bytes) EtherType 0x{ETHERTYPE:04X} "
        f"{operation} prev={prev_phase} next={next_phase}"
    )

    if not args.iface:
        print("no --iface: not sending. Pass an adapter name to transmit 0x88B5.")
        return 0
    try:
        send_raw(args.iface, eth)
        print(f"sent {operation} on {args.iface} to {args.dst}")
    except Exception as e:
        print(f"SEND FAILED (not falling back to TCP JSON): {e}")
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
