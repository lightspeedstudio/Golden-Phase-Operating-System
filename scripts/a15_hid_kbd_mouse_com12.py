#!/usr/bin/env python3
"""COM12 A15: one boot, keyboard then mouse on the single J10 port.

PZ7020 has one USB host port. The stack does not enumerate through a hub
(214b:7250 non-HID). Operator plugs the keyboard directly, then swaps to
the mouse without RESET so one BootSessionRoot holds both.

Raw RX in *.uart. Host records in *.host.jsonl. gpos> is transport.
Does not write PASS/FAIL into UART.
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
from a15_hid_kbd_mouse_board import EXIT_ACCEPT, _keyboard, _mouse, classify  # noqa: E402

PORT = "COM12"
BAUD = 115200


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--evidence-dir", required=True)
    ap.add_argument("--commit", default="")
    ap.add_argument("--elf-sha256", default="")
    ap.add_argument("--boot-sha256", default="")
    ap.add_argument("--bit-sha256", default="")
    ap.add_argument("--swap-seconds", type=float, default=30.0)
    args = ap.parse_args()

    ev = Path(args.evidence_dir)
    ev.mkdir(parents=True, exist_ok=True)
    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    uart_path = ev / f"com12_a15_hid_{stamp}.uart"
    host_out = ev / f"com12_a15_hid_{stamp}.host.jsonl"
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
        f"OPEN {PORT} DTR/RTS off. Plug one HID device into J10 (keyboard or mouse, "
        "not a hub). SD in PZ7020, press RESET. When asked, swap to the other device.",
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

    if not wait_boot(300.0):
        write_host_event(host_out, "no_boot", {"uart_bytes": len(buf)})
        print("FAIL: no boot prompt", flush=True)
        s.close()
        return 1

    send("usb probe", wait_s=40.0)
    listed = send("usb list", wait_s=40.0)
    send("usb hid", wait_s=40.0)
    if _keyboard(listed) and not _mouse(listed):
        print("TYPE ON THE USB KEYBOARD NOW (usb events ~3s, then usb keys ~5s).", flush=True)
        write_host_event(host_out, "operator_keyboard", {"hint": "type on keyboard"})
    elif _mouse(listed) and not _keyboard(listed):
        print("MOVE THE USB MOUSE NOW (usb events ~3s, then usb keys ~5s).", flush=True)
        write_host_event(host_out, "operator_mouse", {"hint": "move mouse"})
    else:
        print("TYPE or MOVE the HID device now (usb events ~3s, then usb keys ~5s).", flush=True)
        write_host_event(
            host_out,
            "operator_hid",
            {"listed_kbd": _keyboard(listed), "listed_mouse": _mouse(listed)},
        )
    send("usb events", wait_s=12.0)
    send("usb keys", wait_s=20.0)

    so_far = bytes(buf).decode("utf-8", "replace")
    need_k = not _keyboard(so_far)
    need_m = not _mouse(so_far)
    if need_k and need_m:
        print("J10 is not a HID keyboard or mouse (hub?). Will still wait for a HID swap.", flush=True)
        write_host_event(host_out, "no_hid", {"listed": listed})
        print(
            f"UNPLUG hub. Plug KEYBOARD or MOUSE into J10. Waiting {args.swap_seconds:.0f}s.",
            flush=True,
        )
    elif need_k:
        print(
            f"UNPLUG this device. Plug the KEYBOARD into J10 (not the hub). "
            f"Waiting {args.swap_seconds:.0f}s.",
            flush=True,
        )
        write_host_event(host_out, "operator_swap_keyboard", {"seconds": args.swap_seconds})
    elif need_m:
        print(
            f"UNPLUG this device. Plug the MOUSE into J10 (not the hub). "
            f"Waiting {args.swap_seconds:.0f}s.",
            flush=True,
        )
        write_host_event(host_out, "operator_swap_mouse", {"seconds": args.swap_seconds})
    if need_k or need_m:
        drain(int(args.swap_seconds * 1000))
        send("usb hid-detach", wait_s=10.0)
        send("usb probe", wait_s=40.0)
        send("usb list", wait_s=40.0)
        send("usb hid", wait_s=40.0)
        if need_k:
            print("TYPE ON THE USB KEYBOARD NOW (usb events ~3s, then usb keys ~5s).", flush=True)
            write_host_event(host_out, "operator_keyboard", {"hint": "type on keyboard"})
        else:
            print("MOVE THE USB MOUSE NOW (usb events ~3s, then usb keys ~5s).", flush=True)
            write_host_event(host_out, "operator_mouse", {"hint": "move mouse"})
        send("usb events", wait_s=12.0)
        send("usb keys", wait_s=20.0)
    s.close()

    uart_bytes = uart_path.read_bytes() if uart_path.exists() else b""
    decoded = uart_bytes.decode("utf-8", "replace")
    code, reason, info = classify(decoded)
    raw, raw_reason = is_raw_uart_evidence(decoded)
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
            "ok": code == EXIT_ACCEPT and raw,
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
    if code != EXIT_ACCEPT or not raw:
        print("FAIL: one-boot keyboard+mouse HID not proven", flush=True)
        return 1
    print("BOARD_PASS A15 HID keyboard and mouse", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
