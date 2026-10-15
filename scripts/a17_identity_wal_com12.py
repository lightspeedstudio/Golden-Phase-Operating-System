#!/usr/bin/env python3
"""COM12 A17: identity /idstore.bin WAL persist, RESET, recover same checksum.

Raw RX in *.uart (persist boot and recover boot separate). Host records in
*.host.jsonl. gpos> is transport. Does not write PASS/FAIL into UART.
Does not log passwd secrets.
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
from a17_identity_wal_board import (  # noqa: E402
    EXIT_ACCEPT,
    EXIT_REJECT,
    classify_persist,
    classify_recover,
)

PORT = "COM12"
BAUD = 115200
# 16 chars, not blocklisted, no product/username substring.
_PW = "A17-idstore-wal1"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--evidence-dir", required=True)
    ap.add_argument("--commit", default="")
    ap.add_argument("--elf-sha256", default="")
    ap.add_argument("--boot-sha256", default="")
    ap.add_argument("--bit-sha256", default="")
    ap.add_argument("--recover-only", action="store_true")
    ap.add_argument("--persist-uart", default="")
    args = ap.parse_args()

    ev = Path(args.evidence_dir)
    ev.mkdir(parents=True, exist_ok=True)
    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    recover_uart = ev / f"com12_a17_recover_{stamp}.uart"
    host_out = ev / f"com12_a17_{stamp}.host.jsonl"
    if args.recover_only:
        if not args.persist_uart:
            print("FAIL: --recover-only needs --persist-uart", flush=True)
            return 1
        persist_uart = Path(args.persist_uart)
        persist_bytes = persist_uart.read_bytes()
        persist_decoded = persist_bytes.decode("utf-8", "replace")
        pcode, preason, info = classify_persist(persist_decoded)
        if pcode != EXIT_ACCEPT:
            print(f"FAIL: persist uart not IDENTITY_WAL_PERSIST: {preason}", flush=True)
            return 1
    else:
        persist_uart = ev / f"com12_a17_persist_{stamp}.uart"
        persist_bytes = b""
        persist_decoded = ""
        pcode, preason, info = EXIT_REJECT, "", {}

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
    if args.recover_only:
        print(
            f"OPEN {PORT} DTR/RTS off. Recover-only vs {persist_uart.name}. "
            "PRESS RESET on PZ7020 now.",
            flush=True,
        )
    else:
        print(
            f"OPEN {PORT} DTR/RTS off. Eject SD from PC, insert in PZ7020, press RESET.",
            flush=True,
        )

    uart_path = recover_uart if args.recover_only else persist_uart
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

    def send_secret(wait_s: float = 90.0) -> str:
        before = len(buf)
        write_host_event(host_out, "send_secret", {"uart": uart_path.name})
        s.write((_PW + "\r").encode("ascii"))
        end = time.time() + wait_s
        while time.time() < end:
            drain(80)
            chunk = buf[before:].decode("utf-8", "replace")
            if "new root password:" in chunk and chunk.count("new root password:") == 1:
                # first prompt consumed; wait for confirm prompt handled by caller
                if "confirm new root password:" in chunk or idle_prompt(bytes(buf)):
                    return chunk
            if idle_prompt(bytes(buf)) and len(buf) > before + 2:
                return chunk
        return buf[before:].decode("utf-8", "replace")

    if not args.recover_only:
        if not wait_boot(300.0):
            write_host_event(host_out, "no_boot", {"phase": "persist", "uart_bytes": len(buf)})
            print("FAIL: no persist-boot prompt", flush=True)
            s.close()
            return 1

        send("fsj status")
        send("fsj format CONFIRM", wait_s=90.0)
        send("fsj status")
        send("fsj check")
        st = send("passwd --status")
        if "unprovisioned" in st:
            write_host_event(host_out, "provision", {"uart": uart_path.name})
            before = len(buf)
            s.write(b"passwd\r")
            end = time.time() + 20.0
            while time.time() < end:
                drain(80)
                if b"new root password:" in buf[before:]:
                    break
            send_secret(wait_s=20.0)
            end = time.time() + 20.0
            while time.time() < end:
                drain(80)
                if b"confirm new root password:" in buf:
                    break
            send_secret(wait_s=90.0)
            send("passwd --status")
        send("hexdump /idstore.bin --length 8")
        send("checksum /idstore.bin")
        send("fsj check")

        persist_bytes = persist_uart.read_bytes() if persist_uart.exists() else b""
        persist_decoded = persist_bytes.decode("utf-8", "replace")
        pcode, preason, info = classify_persist(persist_decoded)
        write_host_event(
            host_out,
            "persist_classify",
            {
                "ok": pcode == EXIT_ACCEPT,
                "reason": preason,
                "run_id": info.get("run_id", ""),
                "uart_sha256": hashlib.sha256(persist_bytes).hexdigest(),
                "uart_bytes": len(persist_bytes),
            },
        )
        print(f"PERSIST_CLASS {preason}", flush=True)
        if pcode != EXIT_ACCEPT:
            print("FAIL: persist boot did not prove identity WAL file", flush=True)
            s.close()
            return 1

        uart_path = recover_uart
        buf = bytearray()
        print("PRESS RESET on PZ7020 for journal recover boot.", flush=True)

    write_host_event(
        host_out,
        "wait_reset",
        {
            "persist_run_id": info.get("run_id", ""),
            "recover_only": bool(args.recover_only),
            "persist_uart": persist_uart.name,
        },
    )
    if not wait_boot(300.0):
        write_host_event(host_out, "no_boot", {"phase": "recover", "uart_bytes": len(buf)})
        print("FAIL: no recover-boot prompt", flush=True)
        s.close()
        return 1

    send("fsj check")
    send("hexdump /idstore.bin --length 8")
    send("checksum /idstore.bin")
    send("passwd --status")
    s.close()

    recover_bytes = recover_uart.read_bytes() if recover_uart.exists() else b""
    recover_decoded = recover_bytes.decode("utf-8", "replace")
    rcode, rreason = classify_recover(recover_decoded, info)
    raw_p, raw_p_reason = is_raw_uart_evidence(persist_decoded)
    raw_r, raw_r_reason = is_raw_uart_evidence(recover_decoded)
    write_host_event(
        host_out,
        "close",
        {
            **bind,
            "persist_uart_sha256": hashlib.sha256(persist_bytes).hexdigest(),
            "recover_uart_sha256": hashlib.sha256(recover_bytes).hexdigest(),
            "persist_raw": raw_p,
            "recover_raw": raw_r,
            "persist_reason": preason,
            "recover_reason": rreason,
            "persist_run_id": info.get("run_id", ""),
            "recover_run_id": (boot_session_roots(recover_decoded) or [""])[0],
            "ok": rcode == EXIT_ACCEPT and raw_p and raw_r,
        },
    )
    print(
        f"EVIDENCE {persist_uart} sha256={hashlib.sha256(persist_bytes).hexdigest()} "
        f"bytes={len(persist_bytes)}",
        flush=True,
    )
    print(
        f"EVIDENCE {recover_uart} sha256={hashlib.sha256(recover_bytes).hexdigest()} "
        f"bytes={len(recover_bytes)}",
        flush=True,
    )
    print(f"HOST {host_out} type=HOST_PARSER_EVENT", flush=True)
    print(f"raw_persist={'YES' if raw_p else 'NO'} {raw_p_reason}", flush=True)
    print(f"raw_recover={'YES' if raw_r else 'NO'} {raw_r_reason}", flush=True)
    print(f"RECOVER_CLASS {rreason}", flush=True)
    if rcode != EXIT_ACCEPT or not raw_p or not raw_r:
        print("FAIL: recover boot did not prove identity WAL redo", flush=True)
        return 1
    print("BOARD_PASS A17 identity snapshot survived SD WAL recover", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
