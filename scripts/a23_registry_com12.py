#!/usr/bin/env python3
"""COM12 A23: install signed GPA, reset, prove AppRegistry reconstruction.

Raw RX in *.uart (install boot and recover boot separate). Host records in
*.host.jsonl. gpos> is transport. Does not write PASS/FAIL into UART.
"""

from __future__ import annotations

import argparse
import hashlib
import re
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
from a23_registry_board import (  # noqa: E402
    A12_PATH,
    A12_PAYLOAD,
    EXIT_ACCEPT,
    INSTALL_RE,
    STORE_FILES,
    classify_install,
    classify_recover,
)

PORT = "COM12"
BAUD = 115200
HEX64 = re.compile(r"\b[0-9a-f]{64}\b")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--evidence-dir", required=True)
    ap.add_argument("--commit", default="")
    ap.add_argument("--elf-sha256", default="")
    ap.add_argument("--boot-sha256", default="")
    ap.add_argument("--bit-sha256", default="")
    ap.add_argument("--gpa-sha256", default="")
    ap.add_argument(
        "--recover-only",
        action="store_true",
        help="Skip install boot; wait for recover boot against --install-uart",
    )
    ap.add_argument("--install-uart", default="")
    args = ap.parse_args()

    ev = Path(args.evidence_dir)
    ev.mkdir(parents=True, exist_ok=True)
    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    recover_uart = ev / f"com12_a23_recover_{stamp}.uart"
    host_out = ev / f"com12_a23_{stamp}.host.jsonl"
    if args.recover_only:
        if not args.install_uart:
            print("FAIL: --recover-only needs --install-uart", flush=True)
            return 1
        install_uart = Path(args.install_uart)
        if not install_uart.is_file():
            print(f"FAIL: install uart missing: {install_uart}", flush=True)
            return 1
        install_bytes = install_uart.read_bytes()
        install_decoded = install_bytes.decode("utf-8", "replace")
        icode, ireason, info = classify_install(install_decoded)
        if icode != EXIT_ACCEPT:
            print(f"FAIL: install uart not INSTALL_A23: {ireason}", flush=True)
            return 1
    else:
        install_uart = ev / f"com12_a23_install_{stamp}.uart"
        install_bytes = b""
        install_decoded = ""
        icode, ireason, info = EXIT_ACCEPT, "", {}
    bind = {
        "commit": args.commit,
        "elf_sha256": args.elf_sha256,
        "boot_sha256": args.boot_sha256,
        "bit_sha256": args.bit_sha256,
        "gpa_sha256": args.gpa_sha256,
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
        write_host_event(
            host_out,
            "install_classify",
            {
                "ok": True,
                "reason": ireason,
                "root": info.get("root", ""),
                "run_id": info.get("run_id", ""),
                "uart_sha256": hashlib.sha256(install_bytes).hexdigest(),
                "uart_bytes": len(install_bytes),
                "reused": True,
            },
        )
        print(
            f"OPEN {PORT} DTR/RTS off. Recover-only vs {install_uart.name}. "
            "PRESS RESET on PZ7020 now.",
            flush=True,
        )
    else:
        print(
            f"OPEN {PORT} DTR/RTS off. Eject SD from PC, insert in PZ7020, press RESET.",
            flush=True,
        )

    uart_path = recover_uart if args.recover_only else install_uart
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

    if not args.recover_only:
        if not wait_boot(300.0):
            write_host_event(
                host_out,
                "no_boot",
                {"phase": "install", "uart_bytes": len(buf)},
            )
            print("FAIL: no install-boot prompt", flush=True)
            s.close()
            return 1

        status = send("fsj status")
        if "SD_PARTITION_1_UNFORMATTED" in status or "unformatted" in status:
            send("fsj format CONFIRM", wait_s=40.0)
            send("fsj status")
        send("fsj check")
        send("txlog")
        send("app list")
        listed = send("ls /phase/packages")
        for hex_id in HEX64.findall(listed):
            store = f"/phase/packages/{hex_id}"
            send(f"ls {store}")
            for name in STORE_FILES:
                send(f"unlock {store}/{name}")
        send("extmap")
        install_text = send("app install hello.gpa", wait_s=40.0)
        inst = INSTALL_RE.search(install_text)
        root = inst.group(2) if inst else ""
        app = inst.group(1) if inst else ""
        write_host_event(host_out, "install_parse", {"root": root, "app": app, "ok": bool(root)})
        if root:
            store = f"/phase/packages/{root}"
            send(f"ls {store}")
            for name in STORE_FILES:
                send(f"checksum {store}/{name}")
        send("txlog")
        send("app list")
        if app:
            send(f"app run {app}", wait_s=40.0)
        send("extmap")
        send(f"write {A12_PATH} {A12_PAYLOAD}")
        send("extmap")
        send(f"cat {A12_PATH}")
        send("fsj check")
        send("fsj status")

        install_bytes = install_uart.read_bytes() if install_uart.exists() else b""
        install_decoded = install_bytes.decode("utf-8", "replace")
        icode, ireason, info = classify_install(install_decoded)
        write_host_event(
            host_out,
            "install_classify",
            {
                "ok": icode == EXIT_ACCEPT,
                "reason": ireason,
                "root": info.get("root", ""),
                "run_id": info.get("run_id", ""),
                "registry_root": info.get("registry_root", ""),
                "generation": info.get("generation", 0),
                "uart_sha256": hashlib.sha256(install_bytes).hexdigest(),
                "uart_bytes": len(install_bytes),
            },
        )
        print(f"INSTALL_CLASS {ireason} root={info.get('root', '')}", flush=True)
        if icode != EXIT_ACCEPT:
            print("FAIL: install boot did not prove GPA+registry+extmap", flush=True)
            s.close()
            return 1

        uart_path = recover_uart
        buf = bytearray()
        print("PRESS RESET on PZ7020 for journal recover boot.", flush=True)

    write_host_event(
        host_out,
        "wait_reset",
        {
            "install_run_id": info["run_id"],
            "recover_only": bool(args.recover_only),
            "install_uart": install_uart.name,
        },
    )
    if not wait_boot(300.0):
        write_host_event(host_out, "no_boot", {"phase": "recover", "uart_bytes": len(buf)})
        print("FAIL: no recover-boot prompt", flush=True)
        s.close()
        return 1

    store = f"/phase/packages/{info['root']}"
    send(f"ls {store}")
    for name in STORE_FILES:
        send(f"checksum {store}/{name}")
    send(f"cat {A12_PATH}")
    send("extmap")
    send("app list")
    send("app reconstruct")
    send("app list")
    send(f"app run {info['app']}", wait_s=40.0)
    send("fsj check")
    send("fsj status")
    send("txlog")
    s.close()

    recover_bytes = recover_uart.read_bytes() if recover_uart.exists() else b""
    recover_decoded = recover_bytes.decode("utf-8", "replace")
    rcode, rreason = classify_recover(recover_decoded, info)
    raw_i, raw_i_reason = is_raw_uart_evidence(install_decoded)
    raw_r, raw_r_reason = is_raw_uart_evidence(recover_decoded)
    write_host_event(
        host_out,
        "close",
        {
            **bind,
            "install_uart_sha256": hashlib.sha256(install_bytes).hexdigest(),
            "recover_uart_sha256": hashlib.sha256(recover_bytes).hexdigest(),
            "install_raw": raw_i,
            "recover_raw": raw_r,
            "install_reason": ireason,
            "recover_reason": rreason,
            "install_run_id": info.get("run_id", ""),
            "recover_run_id": (boot_session_roots(recover_decoded) or [""])[0],
            "ok": rcode == EXIT_ACCEPT and raw_i and raw_r,
        },
    )
    print(
        f"EVIDENCE {install_uart} sha256={hashlib.sha256(install_bytes).hexdigest()} "
        f"bytes={len(install_bytes)}",
        flush=True,
    )
    print(
        f"EVIDENCE {recover_uart} sha256={hashlib.sha256(recover_bytes).hexdigest()} "
        f"bytes={len(recover_bytes)}",
        flush=True,
    )
    print(f"HOST {host_out} type=HOST_PARSER_EVENT", flush=True)
    print(f"raw_install={'YES' if raw_i else 'NO'} {raw_i_reason}", flush=True)
    print(f"raw_recover={'YES' if raw_r else 'NO'} {raw_r_reason}", flush=True)
    print(f"RECOVER_CLASS {rreason}", flush=True)
    if rcode != EXIT_ACCEPT or not raw_i or not raw_r:
        print("FAIL: recover boot did not prove AppRegistry reconstruction", flush=True)
        return 1
    print("BOARD_PASS A23 AppRegistry rebuilt from recovered signed GPA", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
