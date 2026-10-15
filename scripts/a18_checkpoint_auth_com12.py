#!/usr/bin/env python3
"""COM12 A18: ephemeral RAM secret A seals; secret B returns BAD_SIGNATURE.

Raw RX in *.uart. Host records in *.host.jsonl. gpos> is transport.
Does not write PASS/FAIL into UART. Does not log provisioned secrets.
Hardware-bound keys are unavailable on PZ7020. Ephemeral is this boot only.
"""

from __future__ import annotations

import argparse
import hashlib
import secrets
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
from a18_checkpoint_auth_board import EXIT_ACCEPT, classify  # noqa: E402

PORT = "COM12"
BAUD = 115200


def _leaked_in_bytes(data: bytes, secret_hex: str) -> bool:
    low = data.lower()
    hx = secret_hex.strip().lower().encode("ascii")
    if hx and hx in low:
        return True
    try:
        raw = bytes.fromhex(secret_hex.strip())
    except ValueError:
        return False
    return bool(raw) and raw in data


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--evidence-dir", required=True)
    ap.add_argument("--commit", default="")
    ap.add_argument("--elf-sha256", default="")
    ap.add_argument("--boot-sha256", default="")
    ap.add_argument("--bit-sha256", default="")
    ap.add_argument("--elf", default="")
    args = ap.parse_args()

    ev = Path(args.evidence_dir)
    ev.mkdir(parents=True, exist_ok=True)
    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    uart_path = ev / f"com12_a18_{stamp}.uart"
    host_out = ev / f"com12_a18_{stamp}.host.jsonl"
    secret_a = secrets.token_hex(32)
    secret_b = secrets.token_hex(32)
    while secret_b == secret_a:
        secret_b = secrets.token_hex(32)

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
    print(
        f"OPEN {PORT} DTR/RTS off. Eject SD from PC, insert in PZ7020, press RESET.",
        flush=True,
    )

    buf = bytearray()

    def drain(ms: int) -> None:
        end = time.time() + ms / 1000.0
        while time.time() < end:
            n = s.in_waiting
            if n:
                chunk = s.read(n)
                buf.extend(chunk)
                append_uart(uart_path, chunk)
                sys.stdout.buffer.write(chunk)
                sys.stdout.flush()
            time.sleep(0.02)

    def wait_boot(seconds: float) -> bool:
        end = time.time() + seconds
        while time.time() < end:
            drain(80)
            text = bytes(buf).decode("utf-8", "replace")
            roots = boot_session_roots(text)
            if idle_prompt(bytes(buf)) and len(roots) == 1:
                return True
        return False

    def send(cmd: str, wait_s: float = 20.0) -> str:
        before = len(buf)
        write_host_event(host_out, "send", {"cmd": cmd, "uart": uart_path.name})
        s.write((cmd + "\r").encode("ascii"))
        end = time.time() + wait_s
        while time.time() < end:
            drain(80)
            if len(buf) > before + 2 and idle_prompt(bytes(buf)):
                return buf[before:].decode("utf-8", "replace")
        return buf[before:].decode("utf-8", "replace")

    def provision(which: str, secret: str) -> str:
        before = len(buf)
        write_host_event(
            host_out,
            "send_secret",
            {"uart": uart_path.name, "which": which, "redacted": True},
        )
        s.write(b"checkpoint-auth provision\r")
        end = time.time() + 20.0
        while time.time() < end:
            drain(80)
            if b"no echo" in buf[before:]:
                break
        s.write((secret + "\r").encode("ascii"))
        end = time.time() + 30.0
        while time.time() < end:
            drain(80)
            if idle_prompt(bytes(buf)) and len(buf) > before + 2:
                return buf[before:].decode("utf-8", "replace")
        return buf[before:].decode("utf-8", "replace")

    if not wait_boot(300.0):
        write_host_event(host_out, "no_boot", {"uart_bytes": len(buf)})
        print("FAIL: no boot prompt", flush=True)
        s.close()
        return 1

    send("checkpoint-auth status")
    send("checkpoint-auth hardware")
    send("checkpoint-auth seal")
    send("checkpoint-auth verify")
    provision("A", secret_a)
    send("checkpoint-auth status")
    send("checkpoint-auth seal")
    send("checkpoint-auth verify")
    provision("B", secret_b)
    send("checkpoint-auth verify")
    send("checkpoint")
    send("root")
    s.close()

    uart_bytes = uart_path.read_bytes() if uart_path.exists() else b""
    decoded = uart_bytes.decode("utf-8", "replace")
    host_bytes = host_out.read_bytes() if host_out.exists() else b""
    code, reason, info = classify(decoded, secret_a, secret_b)
    raw, raw_reason = is_raw_uart_evidence(decoded)
    uart_leak = _leaked_in_bytes(uart_bytes, secret_a) or _leaked_in_bytes(uart_bytes, secret_b)
    host_leak = _leaked_in_bytes(host_bytes, secret_a) or _leaked_in_bytes(host_bytes, secret_b)
    elf_leak = False
    if args.elf:
        elf_bytes = Path(args.elf).read_bytes()
        elf_leak = _leaked_in_bytes(elf_bytes, secret_a) or _leaked_in_bytes(elf_bytes, secret_b)
        if bytes([0x11] * 32) in elf_bytes:
            elf_leak = True

    write_host_event(
        host_out,
        "close",
        {
            **bind,
            "uart_sha256": hashlib.sha256(uart_bytes).hexdigest(),
            "uart_bytes": len(uart_bytes),
            "raw": raw,
            "reason": reason,
            "run_id": info.get("run_id", ""),
            "secret_redacted": True,
            "uart_secret_leak": uart_leak,
            "host_secret_leak": host_leak,
            "elf_secret_leak": elf_leak,
            "ok": code == EXIT_ACCEPT and raw and not uart_leak and not host_leak and not elf_leak,
        },
    )
    print(
        f"EVIDENCE {uart_path} sha256={hashlib.sha256(uart_bytes).hexdigest()} "
        f"bytes={len(uart_bytes)}",
        flush=True,
    )
    print(f"HOST {host_out} type=HOST_PARSER_EVENT", flush=True)
    print(f"raw={'YES' if raw else 'NO'} {raw_reason}", flush=True)
    print(f"CLASS {reason}", flush=True)
    print(
        f"SECRET_REDACTION uart={'LEAK' if uart_leak else 'CLEAN'} "
        f"host={'LEAK' if host_leak else 'CLEAN'} "
        f"elf={'LEAK' if elf_leak else 'CLEAN'}",
        flush=True,
    )
    if code != EXIT_ACCEPT or not raw or uart_leak or host_leak or elf_leak:
        print("FAIL: boot did not prove secret-backed checkpoint authentication", flush=True)
        return 1
    print(
        "BOARD_PASS A18 secret A verified; secret B BAD_SIGNATURE; ephemeral RAM only",
        flush=True,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
