#!/usr/bin/env python3
"""COM12 listen, no CR. Writes NDJSON to debug-638b5a.log."""
from __future__ import annotations

import json
import time
from pathlib import Path

import serial

LOG = Path(r"C:\k-mmu-iso\debug-638b5a.log")
PORT = "COM12"
WAIT = 60.0


def emit(hid: str, msg: str, data: dict) -> None:
    rec = {
        "sessionId": "638b5a",
        "runId": "post-fix-2",
        "hypothesisId": hid,
        "location": "debug_com12_blue_led.py",
        "message": msg,
        "data": data,
        "timestamp": int(time.time() * 1000),
    }
    # #region agent log
    with LOG.open("a", encoding="utf-8") as f:
        f.write(json.dumps(rec, separators=(",", ":")) + "\n")
    # #endregion


def main() -> int:
    s = serial.Serial()
    s.port = PORT
    s.baudrate = 115200
    s.timeout = 0.05
    s.dsrdtr = False
    s.rtscts = False
    s.dtr = False
    s.rts = False
    try:
        s.open()
    except serial.SerialException as e:
        emit("H5", "open_failed", {"err": str(e)})
        print(f"FAIL open {PORT}: {e}", flush=True)
        return 1
    s.dtr = False
    s.rts = False
    emit("H5", "open_ok", {"dtr": False, "rts": False})
    print("", flush=True)
    print("========================================", flush=True)
    print("  COM12 OPEN. POWER THE BOARD ON NOW.", flush=True)
    print("  60 seconds. Do not type. Do not send CR.", flush=True)
    print("========================================", flush=True)
    print("", flush=True)
    buf = bytearray()
    t0 = time.time()
    last = 0
    while time.time() - t0 < WAIT:
        n = s.in_waiting
        if n:
            chunk = s.read(n)
            buf.extend(chunk)
            emit(
                "H1",
                "uart_chunk",
                {
                    "n": len(chunk),
                    "total": len(buf),
                    "hex": chunk[:64].hex(),
                },
            )
        if len(buf) != last and len(buf) - last >= 16:
            last = len(buf)
        time.sleep(0.02)
    text = buf.decode("utf-8", "replace")
    emit(
        "H4",
        "listen_done",
        {
            "bytes": len(buf),
            "hex": buf[:128].hex(),
            "has_gpos": "gpos>" in text,
            "has_Dbang": "D!" in text,
            "has_MMU": "MMU" in text,
            "has_C10": "C10_WATCHDOG_RESET" in text,
            "has_C10_HALT": "C10_HALT" in text,
            "n_watchdog": text.count("C10_WATCHDOG_RESET"),
            "n_halt": text.count("C10_HALT"),
            "n_splash": text.count("Golden Phase"),
            "n_iso": text.count("MMU: CPU0 isolation"),
            "n_Dbang": text.count("D!"),
            "n_gpos": text.count("gpos>"),
            "n_Pbang": text.count("\nP") + (1 if text.startswith("P") else 0),
            "has_splash": "Golden Phase" in text,
            "has_FSBL": "FSBL" in text or "Xilinx" in text,
            "tail": text[-1200:],
            "g1p_idx": text.find("G1P: Layer-0"),
        },
    )
    print(f"bytes={len(buf)} hex={buf[:128].hex()}", flush=True)
    s.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
