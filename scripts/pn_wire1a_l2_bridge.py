#!/usr/bin/env python3
"""PN-WIRE-1A L2 bridge: physical NIC <-> QEMU -netdev socket.

Forwards raw Ethernet frames between a host NIC (Npcap/scapy) and a QEMU
socket netdev (4-byte big-endian length prefix + frame).

This enables:

  PZ7020 GEM0 --cable--> PC NIC --bridge--> QEMU virtio PNW1 peer

Classification: host L2 bridge for PZ7020-to-QEMU proof.
NOT a two-board PhaseNet seal.

Requires: pip install scapy  (Npcap on Windows; root/AF_PACKET on Linux)
"""

from __future__ import annotations

import argparse
import socket
import struct
import sys
import threading

ETHERTYPE = 0x88B5


def _npcap_dev(nic: dict) -> str:
    guid = str(nic.get("guid", "")).strip()
    if guid.startswith("{") and guid.endswith("}"):
        return f"\\Device\\NPF_{guid}"
    name = nic.get("name")
    return name or ""


def resolve_iface(name_or_mac: str) -> str:
    """Resolve Windows friendly name / MAC to an Npcap device path scapy can open.

    Prefer the physical Intel I225 'Ethernet' adapter, not Hyper-V WSL-Bridge
    (same MAC, virtual) or Wi-Fi.
    """
    try:
        from scapy.arch.windows import get_windows_if_list  # type: ignore
    except Exception:
        return name_or_mac

    want = name_or_mac.strip().lower().replace("-", ":")
    nics = get_windows_if_list()
    skip = (
        "hyper-v",
        "vmware",
        "virtualbox",
        "loopback",
        "bluetooth",
        "wi-fi",
        "wlan",
        "wsl",
        "npcap packet driver",
        "lightweight filter",
        "qos packet",
        "kernel debug",
    )
    prefer = ("i225", "ethernet controller (3)", "ethernet controller")

    ranked: list[tuple[int, dict]] = []
    for nic in nics:
        name = str(nic.get("name", ""))
        desc = str(nic.get("description", ""))
        mac = str(nic.get("mac", "")).lower().replace("-", ":")
        blob = f"{name} {desc}".lower()
        if any(s in blob for s in skip):
            continue
        matched = (
            want == name.lower()
            or want == mac
            or want in blob
            or (want == "ethernet" and name.lower() == "ethernet")
        )
        if not matched:
            continue
        score = 0
        if any(p in blob for p in prefer):
            score += 20
        if name.lower() == "ethernet":
            score += 10
        if "i225" in blob:
            score += 15
        ranked.append((score, nic))

    ranked.sort(key=lambda x: -x[0])
    if ranked:
        return _npcap_dev(ranked[0][1]) or name_or_mac
    return name_or_mac


def send_qemu_frame(sock: socket.socket, frame: bytes) -> None:
    sock.sendall(struct.pack("!I", len(frame)) + frame)


def recv_exact(sock: socket.socket, n: int) -> bytes:
    buf = bytearray()
    while len(buf) < n:
        chunk = sock.recv(n - len(buf))
        if not chunk:
            raise ConnectionError("QEMU socket closed")
        buf.extend(chunk)
    return bytes(buf)


def recv_qemu_frame(sock: socket.socket) -> bytes:
    (length,) = struct.unpack("!I", recv_exact(sock, 4))
    if length == 0 or length > 2048:
        raise ValueError(f"bad qemu frame length {length}")
    return recv_exact(sock, length)


def main() -> int:
    ap = argparse.ArgumentParser(description="PN-WIRE-1A NIC <-> QEMU socket L2 bridge")
    ap.add_argument("--iface", required=True, help='host NIC (e.g. "Ethernet")')
    ap.add_argument("--qemu", default="127.0.0.1:1234", help="QEMU socket host:port")
    ap.add_argument("--filter-88b5", action="store_true", help="only forward EtherType 0x88B5")
    args = ap.parse_args()

    try:
        from scapy.all import Ether, sendp, sniff  # type: ignore
    except ImportError:
        print("Install scapy: pip install scapy (Npcap must be running)", file=sys.stderr)
        return 1

    host, port_s = args.qemu.rsplit(":", 1)
    port = int(port_s)
    iface = resolve_iface(args.iface)
    print(f"[bridge] iface request={args.iface!r} resolved={iface!r}")
    try:
        from scapy.all import get_if_hwaddr  # type: ignore

        print(f"[bridge] iface mac={get_if_hwaddr(iface)}")
    except Exception as e:
        print(f"[bridge] iface mac unresolved: {e}")
    print(f"[bridge] connecting to QEMU socket {host}:{port} ...")
    qs = socket.create_connection((host, port), timeout=10)
    qs.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
    print(f"[bridge] connected; sniffing iface={iface}")
    print("[bridge] PN-WIRE-1A host bridge — not a two-board seal")

    stop = threading.Event()
    stats = {"to_qemu": 0, "to_nic": 0}

    def qemu_to_nic() -> None:
        qs.settimeout(0.5)
        while not stop.is_set():
            try:
                frame = recv_qemu_frame(qs)
            except socket.timeout:
                continue
            except (ConnectionError, OSError, ValueError) as e:
                print(f"[bridge] qemu->nic stop: {e}")
                stop.set()
                break
            if args.filter_88b5 and (len(frame) < 14 or frame[12:14] != bytes([0x88, 0xB5])):
                continue
            sendp(Ether(frame), iface=iface, verbose=False)
            stats["to_nic"] += 1
            if len(frame) >= 14:
                et = (frame[12] << 8) | frame[13]
                print(f"[bridge] qemu->nic len={len(frame)} et=0x{et:04X} #{stats['to_nic']}")

    t = threading.Thread(target=qemu_to_nic, daemon=True)
    t.start()

    def on_pkt(pkt) -> None:
        if stop.is_set():
            return
        raw = bytes(pkt)
        if args.filter_88b5 and (len(raw) < 14 or raw[12:14] != bytes([0x88, 0xB5])):
            return
        try:
            send_qemu_frame(qs, raw)
        except OSError as e:
            print(f"[bridge] nic->qemu error: {e}")
            stop.set()
            return
        stats["to_qemu"] += 1
        if len(raw) >= 14:
            et = (raw[12] << 8) | raw[13]
            print(f"[bridge] nic->qemu len={len(raw)} et=0x{et:04X} #{stats['to_qemu']}")

    try:
        sniff(
            iface=iface,
            prn=on_pkt,
            store=False,
            promisc=True,
            stop_filter=lambda _: stop.is_set(),
        )
    except KeyboardInterrupt:
        pass
    finally:
        stop.set()
        try:
            qs.close()
        except OSError:
            pass
        print(f"[bridge] done to_qemu={stats['to_qemu']} to_nic={stats['to_nic']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
