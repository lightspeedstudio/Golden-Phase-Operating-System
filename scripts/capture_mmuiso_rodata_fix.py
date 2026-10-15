#!/usr/bin/env python3
"""COM12 listen for the rodata/data page-alignment fix image.

A04: hashed evidence is append-only COM12 RX (*.uart). Host records are
HOST_PARSER_EVENT in *.host.jsonl and must not carry `full` UART or mint
PASS_BOOT from n_gpos / n_g1p_done / n_aligned_yes counts.

Opens COM12 with DTR and RTS low, sends nothing, streams UART live.
"""
from __future__ import annotations

import hashlib
import sys
import time
from datetime import datetime
from pathlib import Path

import serial

_SCRIPT_DIR = Path(__file__).resolve().parent
if str(_SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(_SCRIPT_DIR))
from a04_rodata_uart import append_uart, classify_rodata_boot_uart, write_host_event

PORT = "COM12"
BAUD = 115200
WAIT = 60.0
ROOT = Path(__file__).resolve().parent.parent
EVIDENCE = ROOT / "fpga" / "zynq" / "evidence" / "kairos_zynq_boot" / "BOOT_MMU_ISOLATION_RODATA_FIX_20260816"


def main() -> int:
    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    uart_out = EVIDENCE / f"com12_rodata_{stamp}.uart"
    host_out = EVIDENCE / f"com12_rodata_{stamp}.host.jsonl"

    s = serial.Serial()
    s.port = PORT
    s.baudrate = BAUD
    s.timeout = 0.05
    s.dsrdtr = False
    s.rtscts = False
    s.dtr = False
    s.rts = False
    try:
        s.open()
    except serial.SerialException as e:
        write_host_event(host_out, "open_failed", {"err": str(e), "port": PORT})
        print(f"FAIL open {PORT}: {e}", flush=True)
        print("Close any other terminal holding COM12, then retry.", flush=True)
        return 1
    s.dtr = False
    s.rts = False
    write_host_event(host_out, "open_ok", {"dtr": False, "rts": False, "port": PORT})

    print("", flush=True)
    print("=" * 60, flush=True)
    print("  COM12 IS OPEN (DTR low, RTS low, nothing sent).", flush=True)
    print("  >>> POWER THE BOARD ON NOW <<<", flush=True)
    print(f"  Listening {WAIT:.0f}s. Do not type. Do not press RESET.", flush=True)
    print("=" * 60, flush=True)
    print("", flush=True)

    buf = bytearray()
    t0 = time.time()
    interrupted = False
    try:
        while time.time() - t0 < WAIT:
            n = s.in_waiting
            if n:
                chunk = s.read(n)
                buf.extend(chunk)
                append_uart(uart_out, chunk)
                sys.stdout.write(chunk.decode("utf-8", "replace"))
                sys.stdout.flush()
            time.sleep(0.02)
    except KeyboardInterrupt:
        interrupted = True
        print("\n[interrupted]", flush=True)
    s.close()

    uart_bytes = uart_out.read_bytes() if uart_out.exists() else b""
    text = uart_bytes.decode("utf-8", "replace")
    code, reason, run_id = classify_rodata_boot_uart(text)
    digest = hashlib.sha256(uart_bytes).hexdigest()
    write_host_event(
        host_out,
        "close",
        {
            "uart_bytes": len(uart_bytes),
            "uart_sha256": digest,
            "interrupted": interrupted,
            "a04_exit": code,
            "reason": reason,
            "run_id": run_id,
        },
    )
    print("", flush=True)
    print("=" * 60, flush=True)
    print(f"  EVIDENCE {uart_out} sha256={digest}", flush=True)
    print(f"  HOST {host_out} type=HOST_PARSER_EVENT", flush=True)
    print(f"  A04_ACCEPT={'YES' if code == 0 else 'NO'} reason={reason}", flush=True)
    print("=" * 60, flush=True)
    return code


if __name__ == "__main__":
    raise SystemExit(main())
