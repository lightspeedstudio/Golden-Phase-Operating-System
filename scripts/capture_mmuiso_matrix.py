#!/usr/bin/env python3
"""COM12 MMU 10/10 matrix. Raw RX in *.uart; host records in *.host.jsonl.

Sends `mmuiso matrix` once after one boot. gpos> is transport only.
Does not send status, version, or named probes.
"""

from __future__ import annotations

import argparse
import hashlib
import sys
import time
from datetime import datetime
from pathlib import Path

import serial

_SCRIPTS = Path(__file__).resolve().parent
_JOURNAL = _SCRIPTS.parent / "fpga" / "zynq" / "research" / "phasefs-persistent-journal-v1"
sys.path.insert(0, str(_SCRIPTS))
sys.path.insert(0, str(_JOURNAL))

from a01_raw_uart import (  # noqa: E402
    append_uart,
    boot_session_roots,
    idle_prompt,
    is_raw_uart_evidence,
    write_host_event,
)
from a03_mmuiso_matrix import EXIT_ACCEPT, classify_mmuiso_matrix  # noqa: E402

PORT = "COM12"
BAUD = 115200
DONE = b"mmuiso matrix done"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--evidence-dir", required=True)
    ap.add_argument("--commit", default="")
    ap.add_argument("--elf-sha256", default="")
    ap.add_argument("--boot-sha256", default="")
    ap.add_argument("--bit-sha256", default="")
    args = ap.parse_args()

    ev = Path(args.evidence_dir)
    ev.mkdir(parents=True, exist_ok=True)
    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    uart_out = ev / f"com12_mmuiso_matrix_{stamp}.uart"
    host_out = ev / f"com12_mmuiso_matrix_{stamp}.host.jsonl"
    bind = {
        "commit": args.commit,
        "elf_sha256": args.elf_sha256,
        "boot_sha256": args.boot_sha256,
        "bit_sha256": args.bit_sha256,
        "port": PORT,
        "dtr": False,
        "rts": False,
    }

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
        write_host_event(host_out, "open_failed", {**bind, "err": str(e)})
        print(f"FAIL: cannot open {PORT}: {e}", flush=True)
        return 1
    s.dtr = False
    s.rts = False
    write_host_event(host_out, "open_ok", bind)
    print(f"OPEN {PORT} DTR/RTS off. Insert SD in PZ7020, press RESET.", flush=True)

    buf = bytearray()

    def drain(ms: int) -> None:
        end = time.time() + ms / 1000.0
        while time.time() < end:
            n = s.in_waiting
            if n:
                chunk = s.read(n)
                buf.extend(chunk)
                append_uart(uart_out, chunk)
                sys.stdout.buffer.write(chunk)
                sys.stdout.flush()
            time.sleep(0.02)

    def wait_boot(seconds: float) -> bool:
        end = time.time() + seconds
        while time.time() < end:
            drain(80)
            text = bytes(buf).decode("utf-8", "replace")
            roots = boot_session_roots(text)
            if (
                b"MMU: CPU0 isolation" in buf
                and idle_prompt(bytes(buf))
                and len(roots) == 1
            ):
                return True
        return False

    if not wait_boot(300.0):
        write_host_event(
            host_out,
            "no_boot",
            {"uart_bytes": len(buf), "idle_prompt": idle_prompt(bytes(buf))},
        )
        if uart_out.exists():
            digest = hashlib.sha256(uart_out.read_bytes()).hexdigest()
            print(f"EVIDENCE {uart_out} sha256={digest}", flush=True)
        print("FAIL: no single BootSessionRoot + isolation banner + prompt", flush=True)
        s.close()
        return 1

    before_len = len(buf)
    write_host_event(
        host_out,
        "send",
        {"cmd": "mmuiso matrix", "before_len": before_len},
    )
    s.write(b"mmuiso matrix\r")

    deadline = time.time() + 90.0
    saw_done = False
    while time.time() < deadline:
        drain(80)
        if DONE in buf:
            saw_done = True
            if idle_prompt(bytes(buf)) and len(buf) > before_len + 8:
                break
    s.close()

    uart_bytes = uart_out.read_bytes() if uart_out.exists() else b""
    text = uart_bytes.decode("utf-8", "replace")
    digest = hashlib.sha256(uart_bytes).hexdigest()
    raw_ok, raw_reason = is_raw_uart_evidence(text)
    code, reason, run_id = classify_mmuiso_matrix(text)
    transport = idle_prompt(uart_bytes)
    write_host_event(
        host_out,
        "close",
        {
            **bind,
            "uart_bytes": len(uart_bytes),
            "uart_sha256": digest,
            "raw_uart": raw_ok,
            "raw_reason": raw_reason,
            "a03_exit": code,
            "a03_reason": reason,
            "run_id": run_id,
            "saw_done": saw_done,
            "transport_prompt": transport,
        },
    )
    print(f"EVIDENCE {uart_out} sha256={digest} bytes={len(uart_bytes)}", flush=True)
    print(f"HOST {host_out} type=HOST_PARSER_EVENT", flush=True)
    print(f"raw_uart={'YES' if raw_ok else 'NO'} reason={raw_reason}", flush=True)
    print(f"A03_ACCEPT={'YES' if code == EXIT_ACCEPT else 'NO'} reason={reason} run_id={run_id}", flush=True)
    print(f"transport_prompt={'YES' if transport else 'NO'} (not a matrix PASS)", flush=True)
    if not raw_ok:
        return 1
    return code


if __name__ == "__main__":
    raise SystemExit(main())
