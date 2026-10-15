#!/usr/bin/env python3
"""Capture net wire phyloop and write NDJSON debug logs for session 2a2f53.

Writes to workspace: debug-2a2f53.log
Parses board lines starting with '#dbg '.
"""

from __future__ import annotations

import json
import re
import sys
import time
from pathlib import Path

LOG_PATH = Path(__file__).resolve().parents[2] / "debug-2a2f53.log"
SESSION = "2a2f53"


def emit(hypothesis_id: str, location: str, message: str, data: dict) -> None:
    rec = {
        "sessionId": SESSION,
        "runId": "phyloop-instrument",
        "hypothesisId": hypothesis_id,
        "location": location,
        "message": message,
        "data": data,
        "timestamp": int(time.time() * 1000),
    }
    with LOG_PATH.open("a", encoding="utf-8") as f:
        f.write(json.dumps(rec, separators=(",", ":")) + "\n")


def main() -> int:
    try:
        import serial
    except ImportError:
        print("pip install pyserial", file=sys.stderr)
        return 1

    port = sys.argv[1] if len(sys.argv) > 1 else "COM12"
    ser = serial.Serial(port, 115200, timeout=0.25, write_timeout=2)
    ser.dtr = True
    ser.rts = True
    time.sleep(0.3)
    ser.reset_input_buffer()
    buf = ""
    ser.write(b"\r")
    t0 = time.time()
    while time.time() - t0 < 45:
        chunk = ser.read(4096)
        if chunk:
            buf += chunk.decode("utf-8", "replace")
            sys.stdout.write(chunk.decode("ascii", "replace"))
            sys.stdout.flush()
            if "gpos> " in buf:
                break
    else:
        emit("Z", "uart", "no_gpos", {"buf_tail": buf[-200:]})
        ser.close()
        return 2

    for cmd in ("net wire init", "net wire phyloop"):
        before = buf.count("gpos> ")
        print(f"\n>>> {cmd}", flush=True)
        ser.write((cmd + "\r").encode("ascii"))
        d = time.time() + 50
        while time.time() < d:
            chunk = ser.read(4096)
            if chunk:
                t = chunk.decode("utf-8", "replace")
                buf += t
                sys.stdout.write(t.encode("ascii", "replace").decode("ascii"))
                sys.stdout.flush()
                if buf.count("gpos> ") > before:
                    break

    ser.close()

    dbg_re = re.compile(r"#dbg hyp=(\w+) key=(\S+) (.*)")
    for line in buf.splitlines():
        m = dbg_re.search(line)
        if not m:
            continue
        hyp, key, rest = m.group(1), m.group(2), m.group(3)
        data = {"raw": rest.strip(), "key": key}
        for tok in rest.split():
            if "=" in tok:
                k, v = tok.split("=", 1)
                data[k] = v
        emit(hyp, f"phyloop:{key}", key, data)

    # Summary markers
    if "RX: MATCH" in buf:
        emit("E", "phyloop:verdict", "MATCH", {"matched": True})
    elif "frames seen but NO MATCH" in buf:
        emit("E", "phyloop:verdict", "OTHER_RX", {"matched": False, "any_rx": True})
    elif "RX: NO MATCH" in buf:
        emit("E", "phyloop:verdict", "NO_MATCH", {"matched": False, "any_rx": False})

    print(f"\n[host] wrote NDJSON to {LOG_PATH}", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
