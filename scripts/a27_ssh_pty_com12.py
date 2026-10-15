#!/usr/bin/env python3
"""COM12 A27: interactive SSH PTY with a real OpenSSH client.

Runtime pubkey via `net ssh key` (not compiled in, not committed).
Client log is host evidence. UART is board bytes only.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import subprocess
import sys
import threading
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
from a27_ssh_pty_board import EXIT_ACCEPT, classify  # noqa: E402

PORT = "COM12"
BAUD = 115200
BOARD = "192.168.5.50"
SSH_PORT = 2222


def ed25519_raw_hex(pub_path: Path) -> str:
    line = pub_path.read_text(encoding="ascii").strip().split()
    blob = base64.b64decode(line[1])
    # uint32 len + "ssh-ed25519" + uint32 len + 32-byte key
    i = 0
    n = int.from_bytes(blob[i : i + 4], "big")
    i += 4
    algo = blob[i : i + n]
    i += n
    if algo != b"ssh-ed25519":
        raise ValueError(f"not ed25519: {algo!r}")
    n = int.from_bytes(blob[i : i + 4], "big")
    i += 4
    key = blob[i : i + n]
    if len(key) != 32:
        raise ValueError("ed25519 key not 32 bytes")
    return key.hex()


def ssh_base(identity: Path, debug: Path | None = None) -> list[str]:
    cmd = ["ssh"]
    if debug is not None:
        cmd += ["-E", str(debug), "-vvv"]
    cmd += [
        "-tt",
        "-i",
        str(identity),
        "-p",
        str(SSH_PORT),
        "-o",
        "StrictHostKeyChecking=no",
        "-o",
        "UserKnownHostsFile=NUL",
        "-o",
        "IdentitiesOnly=yes",
        "-o",
        "PreferredAuthentications=publickey",
        "-o",
        "NumberOfPasswordPrompts=0",
        "-o",
        "BatchMode=yes",
        "-o",
        "ConnectTimeout=8",
        f"kairos@{BOARD}",
    ]
    return cmd


def run_pty(identity: Path, log: Path, pump=None, debug: Path | None = None) -> str:
    cmd = ssh_base(identity, debug=debug)
    proc = subprocess.Popen(
        cmd,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        bufsize=0,
    )
    chunks: list[bytes] = []

    def reader() -> None:
        assert proc.stdout is not None
        while True:
            b = proc.stdout.read(1)
            if not b:
                break
            chunks.append(b)

    def idle(seconds: float) -> None:
        end = time.time() + seconds
        while time.time() < end:
            if pump is not None:
                pump()
            time.sleep(0.05)

    def send_bytes(data: bytes) -> bool:
        if proc.poll() is not None or proc.stdin is None:
            return False
        try:
            proc.stdin.write(data)
            proc.stdin.flush()
            return True
        except OSError:
            return False

    t = threading.Thread(target=reader, daemon=True)
    t.start()
    got = False
    deadline = time.time() + 12.0
    while time.time() < deadline:
        if pump is not None:
            pump()
        text = b"".join(chunks).decode("utf-8", "replace")
        if "gpos>" in text:
            got = True
            break
        if proc.poll() is not None:
            break
        time.sleep(0.1)
    if got:
        send_bytes(b"checkpoint\n")
        idle(1.5)
        send_bytes(b"fsj format\n")
        idle(1.5)
        send_bytes(b"whoami\n")
        idle(1.5)
        send_bytes(b"\x03")
        try:
            proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(timeout=5)
    elif proc.poll() is None:
        proc.kill()
        proc.wait(timeout=5)
    t.join(timeout=2)
    out = b"".join(chunks).decode("utf-8", "replace")
    log.write_text(out, encoding="utf-8")
    return out


def run_neg(identity: Path, log: Path) -> str:
    cmd = [c for c in ssh_base(identity) if c != "-tt"]
    cmd += ["-T", "system status"]
    try:
        p = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=12,
        )
        blob = (p.stdout or "") + (p.stderr or "")
        if p.returncode != 0:
            blob += "\nAUTH_FAIL\n"
    except subprocess.TimeoutExpired as e:
        blob = (e.stdout or "") + (e.stderr or "")
        blob += "\nNEG_TIMEOUT\n"
    log.write_text(blob, encoding="utf-8")
    return blob


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
    uart_path = ev / f"com12_a27_ssh_{stamp}.uart"
    host_out = ev / f"com12_a27_ssh_{stamp}.host.jsonl"
    client_log = ev / f"com12_a27_ssh_{stamp}.client.txt"
    neg_log = ev / f"com12_a27_ssh_{stamp}.neg.txt"
    recon_log = ev / f"com12_a27_ssh_{stamp}.reconnect.txt"
    key_ok = ev / "a27_ed25519"
    key_bad = ev / "a27_ed25519_bad"
    bind = {
        "commit": args.commit,
        "elf_sha256": args.elf_sha256,
        "boot_sha256": args.boot_sha256,
        "bit_sha256": args.bit_sha256,
        "port": PORT,
        "dtr": False,
        "rts": False,
    }

    for k in (key_ok, key_bad):
        if k.exists():
            k.unlink()
        pub = Path(str(k) + ".pub")
        if pub.exists():
            pub.unlink()
        subprocess.run(
            ["ssh-keygen", "-t", "ed25519", "-f", str(k), "-N", "", "-q"],
            check=True,
        )
    hex_ok = ed25519_raw_hex(Path(str(key_ok) + ".pub"))

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
        f"OPEN {PORT} DTR/RTS off. I225 <-> U15, host 192.168.5.2. "
        "SD in PZ7020, press RESET.",
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

    send(f"net ssh key {hex_ok}")
    write_host_event(host_out, "ssh_key_set", {"hex": hex_ok})

    before = len(buf)
    write_host_event(host_out, "send", {"cmd": f"net ssh serve {SSH_PORT} 90"})
    s.write(f"net ssh serve {SSH_PORT} 90\r".encode("ascii"))
    started = False
    end = time.time() + 25.0
    while time.time() < end:
        drain(80)
        chunk = buf[before:].decode("utf-8", "replace")
        if "starting SSH server" in chunk:
            started = True
            break
    if not started:
        print("FAIL: SSH server did not start", flush=True)
        s.close()
        return 1

    client_blob = ""
    recon_blob = ""
    neg_blob = ""
    # Do not raw-TCP-probe: that consumes the sole smoltcp listener and used
    # to leave sunset's Runner on a dead session. OpenSSH is the first connect.
    time.sleep(1.0)
    write_host_event(host_out, "ssh_try", {"host": BOARD, "port": SSH_PORT})
    client_blob = ""
    ssh_deadline = time.time() + 20.0
    attempt = 0
    while time.time() < ssh_deadline:
        attempt += 1
        dbg = ev / f"{uart_path.stem}.pty{attempt}.sshdebug"
        client_blob = run_pty(
            key_ok,
            client_log,
            pump=lambda: drain(80),
            debug=dbg,
        )
        write_host_event(
            host_out,
            "ssh_attempt",
            {
                "n": attempt,
                "client_bytes": len(client_blob),
                "got_prompt": "gpos>" in client_blob,
                "debug": dbg.name,
            },
        )
        if "gpos>" in client_blob:
            break
        drain(80)
        time.sleep(0.4)
    if "gpos>" in client_blob:
        write_host_event(host_out, "pty_open", {"bytes": len(client_blob)})
        # One PTY is enough for CLASS SSH_PTY. A second interactive session
        # rebooted the board before serve printed its summary.
        drain(80)
        time.sleep(1.0)
        drain(80)
        neg_blob = run_neg(key_bad, neg_log)
    else:
        write_host_event(
            host_out,
            "pty_fail",
            {"client_bytes": len(client_blob)},
        )
        print("FAIL: OpenSSH -tt got no gpos>", flush=True)

    serve_end = time.time() + 95.0
    splash = b"Golden Phase Operating System boot splash"
    while time.time() < serve_end:
        drain(80)
        raw = bytes(buf)
        if idle_prompt(raw) and b"net ssh: served" in raw:
            break
        if raw.count(splash) >= 2:
            write_host_event(host_out, "board_reboot", {"uart_bytes": len(buf)})
            print("FAIL: board rebooted during SSH serve", flush=True)
            break
    send("checkpoint")
    s.close()

    uart_bytes = uart_path.read_bytes() if uart_path.exists() else b""
    decoded = uart_bytes.decode("utf-8", "replace")
    client_all = client_blob + "\n" + recon_blob + "\n" + neg_blob
    code, reason, info = classify(decoded, client_all)
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
    print(f"CLIENT {client_log}", flush=True)
    print(f"raw={'YES' if raw else 'NO'} {raw_reason}", flush=True)
    print(f"CLASS {reason}", flush=True)
    if code != EXIT_ACCEPT or not raw:
        print("FAIL: SSH PTY not proven", flush=True)
        return 1
    print("BOARD_PASS A27 interactive SSH PTY", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
