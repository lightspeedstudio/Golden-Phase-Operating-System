#!/usr/bin/env python3
"""One-shot PN-WIRE-1 L2 handshake diagnostic (Windows/Linux).

Starts host peer on physical Ethernet (I225), runs board:
  net wire init
  net wire ping

Prints a single verdict. Does NOT claim PhaseNet PASS.
Does NOT fix missing RGMII/PHY TX — if PEER_HEARD_REQ=no, next step is U15 A/B/C.

  python scripts/pn_wire1_handshake_run.py --iface Ethernet --port COM12
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import re
import subprocess
import sys
import time
from datetime import datetime
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BOARD_SRC = "02:00:00:47:01:50"


def load_resolve():
    path = Path(__file__).with_name("pn_wire1a_l2_bridge.py")
    spec = importlib.util.spec_from_file_location("bridge", path)
    mod = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(mod)
    return mod.resolve_iface


def evidence_dir() -> Path:
    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    d = ROOT / "fpga" / "zynq" / "evidence" / "kairos_zynq_boot" / f"pn_wire1_handshake_{stamp}"
    d.mkdir(parents=True, exist_ok=True)
    return d


def run_uart(port: str, baud: int, cmds: list[str], boot_timeout: float) -> str:
    try:
        import serial  # type: ignore
    except ImportError:
        print("pip install pyserial", file=sys.stderr)
        raise SystemExit(1)

    ser = serial.Serial(port, baud, timeout=0.25, write_timeout=2)
    ser.dtr = True
    ser.rts = True
    time.sleep(0.25)
    ser.reset_input_buffer()
    buf = ""
    deadline = time.time() + boot_timeout
    last = 0.0
    print(f"[hs] wait gpos> on {port} ({boot_timeout:.0f}s)", flush=True)
    while time.time() < deadline:
        chunk = ser.read(4096)
        if chunk:
            t = chunk.decode("utf-8", "replace")
            buf += t
            sys.stdout.write(t.encode("ascii", "replace").decode("ascii"))
            sys.stdout.flush()
            if "gpos> " in buf:
                break
        if time.time() - last > 3.0 and "gpos>" not in buf:
            ser.write(b"\r")
            last = time.time()
    else:
        ser.close()
        raise RuntimeError("no gpos> prompt")

    def cmd(c: str, timeout: float = 90.0) -> None:
        nonlocal buf
        before = buf.count("gpos> ")
        print(f"\n>>> {c}", flush=True)
        ser.write((c + "\r").encode("ascii"))
        d = time.time() + timeout
        while time.time() < d:
            chunk = ser.read(4096)
            if chunk:
                t = chunk.decode("utf-8", "replace")
                buf += t
                sys.stdout.write(t.encode("ascii", "replace").decode("ascii"))
                sys.stdout.flush()
                if buf.count("gpos> ") > before:
                    return
        raise TimeoutError(f"timeout waiting for prompt after: {c}")

    for c in cmds:
        # wire ping can take a while waiting for REP
        cmd(c, timeout=120.0 if "ping" in c else 60.0)
    ser.close()
    return buf


def main() -> int:
    ap = argparse.ArgumentParser(description="PN-WIRE-1 L2 handshake diagnostic")
    ap.add_argument("--iface", default="Ethernet", help="PC NIC friendly name")
    ap.add_argument("--port", default="COM12")
    ap.add_argument("--baud", type=int, default=115200)
    ap.add_argument("--peer-timeout", type=float, default=45.0)
    ap.add_argument("--boot-timeout", type=float, default=70.0)
    ap.add_argument("--capture-seconds", type=float, default=40.0)
    args = ap.parse_args()

    resolve = load_resolve()
    try:
        resolved = resolve(args.iface)
    except Exception as e:
        print(f"[hs] iface resolve failed: {e}", file=sys.stderr)
        return 1

    ev = evidence_dir()
    peer_log = ev / "peer.txt"
    uart_log = ev / "com12.txt"
    cap_log = ev / "capture.txt"
    verdict_path = ev / "VERDICT.txt"

    print(f"[hs] evidence={ev}")
    print(f"[hs] iface request={args.iface!r} resolved={resolved!r}")
    print("[hs] starting host peer (scapy) + optional capture", flush=True)

    peer_cmd = [
        sys.executable,
        str(Path(__file__).with_name("pn_wire1_host_peer.py")),
        "--iface",
        args.iface,
        "--backend",
        "scapy",
        "--count",
        "1",
        "--timeout",
        str(args.peer_timeout),
    ]
    cap_cmd = [
        sys.executable,
        str(Path(__file__).with_name("pn_wire1_tx_capture.py")),
        "--iface",
        args.iface,
        "--seconds",
        str(args.capture_seconds),
    ]

    peer_proc = subprocess.Popen(
        peer_cmd,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    cap_proc = subprocess.Popen(
        cap_cmd,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        encoding="utf-8",
        errors="replace",
    )

    # Let Npcap open before board TX
    time.sleep(2.5)

    uart_err = None
    buf = ""
    try:
        buf = run_uart(
            args.port,
            args.baud,
            ["net wire init", "net wire ping"],
            args.boot_timeout,
        )
    except Exception as e:
        uart_err = str(e)
        print(f"[hs] UART error: {e}", flush=True)

    # Wait for peer/capture to finish
    peer_out = ""
    try:
        peer_out, _ = peer_proc.communicate(timeout=max(5.0, args.peer_timeout + 5))
    except subprocess.TimeoutExpired:
        peer_proc.kill()
        peer_out, _ = peer_proc.communicate()
    cap_out = ""
    try:
        cap_out, _ = cap_proc.communicate(timeout=max(5.0, args.capture_seconds + 5))
    except subprocess.TimeoutExpired:
        cap_proc.kill()
        cap_out, _ = cap_proc.communicate()

    peer_log.write_text(peer_out or "", encoding="utf-8")
    uart_log.write_text(buf + (f"\n[uart_error] {uart_err}\n" if uart_err else ""), encoding="utf-8")
    cap_log.write_text(cap_out or "", encoding="utf-8")

    heard = "REQ OK" in (peer_out or "") or bool(re.search(r"REQ OK", peer_out or ""))
    sent = "REP sent" in (peer_out or "")
    board_pass = "gem wire ping: PASS" in buf
    board_fail = "gem wire ping: FAIL" in buf
    m = re.search(r"board_mac=(\d+)\s+ethertype_88b5=(\d+)", cap_out or "")
    board_mac_count = int(m.group(1)) if m else -1
    et_count = int(m.group(2)) if m else -1

    board_ping = "PASS" if board_pass else ("FAIL" if board_fail or uart_err else "UNKNOWN")

    lines = [
        "PN-WIRE-1 L2 handshake diagnostic (not PhaseNet)",
        f"evidence_dir={ev}",
        f"iface_request={args.iface}",
        f"iface_resolved={resolved}",
        f"PEER_HEARD_REQ={'yes' if heard else 'no'}",
        f"PEER_SENT_REP={'yes' if sent else 'no'}",
        f"BOARD_PING={board_ping}",
        f"PC_BOARD_MAC_COUNT={board_mac_count}",
        f"PC_ETHERTYPE_88B5_COUNT={et_count}",
    ]
    if board_ping != "PASS" or not heard:
        lines.append("VERDICT=FAIL — handshake did not complete")
        lines.append("NEXT=U15 scope A/B/C during net wire txburst (no more GEM/handshake software)")
        lines.append("NOTE=Handshake cannot fix missing L2 TX to the PC NIC")
    else:
        lines.append("VERDICT=PASS — REQ heard, REP sent, board roundtrip OK (still not PhaseNet)")

    verdict = "\n".join(lines) + "\n"
    verdict_path.write_text(verdict, encoding="utf-8")
    print("\n======== HANDSHAKE VERDICT ========")
    print(verdict)
    print("===================================")

    # Machine-readable sidecar
    (ev / "verdict.json").write_text(
        json.dumps(
            {
                "PEER_HEARD_REQ": heard,
                "PEER_SENT_REP": sent,
                "BOARD_PING": board_ping,
                "PC_BOARD_MAC_COUNT": board_mac_count,
                "PC_ETHERTYPE_88B5_COUNT": et_count,
                "src_mac": BOARD_SRC,
                "ethertype": "0x88B5",
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )

    return 0 if (heard and sent and board_pass) else 2


if __name__ == "__main__":
    sys.exit(main())
