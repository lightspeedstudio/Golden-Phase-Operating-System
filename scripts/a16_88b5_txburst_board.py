#!/usr/bin/env python3
"""A16 0x88B5 classifier: UART n=32 TX plus host NIC 32/32.

Does not open COM. Does not modify kernel output.
Host counts must live in the sidecar, never in UART.
UART-only txburst OK is REJECT.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

_SCRIPTS = Path(__file__).resolve().parent
_JOURNAL = _SCRIPTS.parent / "fpga" / "zynq" / "research" / "phasefs-persistent-journal-v1"
sys.path.insert(0, str(_SCRIPTS))
sys.path.insert(0, str(_JOURNAL))

from a01_raw_uart import (  # noqa: E402
    boot_session_roots,
    is_raw_uart_evidence,
)

EXIT_ACCEPT = 0
EXIT_REJECT = 1

TXBURST = re.compile(
    r"gem wire txburst: OK n=32 ethertype=0x88B5 "
    r"dst=ff:ff:ff:ff:ff:ff src=02:00:00:47:01:50"
)
HOST_LINE = re.compile(r"ethertype_88b5=(\d+)\s+board_mac=(\d+)")
HOST_LINE_ALT = re.compile(r"board_mac=(\d+)\s+ethertype_88b5=(\d+)")
SEQ_RUN = re.compile(r"seq_run=(\d+)")


def _forbidden(text: str) -> str | None:
    if re.search(r"^result=PASS\s*$", text, flags=re.M):
        return "HOST_RESULT_LINE"
    if re.search(r"^===== .+ (PASS|FAIL)", text, flags=re.M):
        return "HOST_STEP_BANNER"
    return None


def parse_host_counts(host_text: str) -> tuple[int, int]:
    """Best ethertype_88b5 / board_mac pair from jsonl or txcap text."""
    best_et = 0
    best_mac = 0
    for line in host_text.splitlines():
        line = line.strip()
        if not line:
            continue
        if line.startswith("{"):
            try:
                rec = json.loads(line)
            except json.JSONDecodeError:
                continue
            data = rec.get("data", rec)
            if not isinstance(data, dict):
                continue
            et = int(data.get("ethertype_88b5", 0) or 0)
            mac = int(data.get("board_mac", 0) or 0)
            if et > best_et or (et == best_et and mac > best_mac):
                best_et, best_mac = et, mac
            continue
        m = HOST_LINE.search(line)
        if m:
            et, mac = int(m.group(1)), int(m.group(2))
            if et > best_et or (et == best_et and mac > best_mac):
                best_et, best_mac = et, mac
            continue
        m = HOST_LINE_ALT.search(line)
        if m:
            mac, et = int(m.group(1)), int(m.group(2))
            if et > best_et or (et == best_et and mac > best_mac):
                best_et, best_mac = et, mac
    return best_et, best_mac


LINK_SPEED = re.compile(r"gem link:.*\bspeed=(\d+)\s+duplex=full")


def classify(
    uart: str, host: str, require_speed: int | None = None
) -> tuple[int, str, dict]:
    bad = _forbidden(uart)
    if bad:
        return EXIT_REJECT, bad, {}
    ok, reason = is_raw_uart_evidence(uart)
    if not ok:
        return EXIT_REJECT, reason, {}
    roots = boot_session_roots(uart)
    if len(roots) != 1:
        return EXIT_REJECT, "BOOT_EPOCH", {"roots": roots}
    info = {"run_id": roots[0]}
    if not TXBURST.search(uart):
        return EXIT_REJECT, "NO_TXBURST_32", info
    if require_speed == 1000 and len(TXBURST.findall(uart)) != 1:
        return EXIT_REJECT, "MULTI_TXBURST", info
    if "ethertype_88b5=" in uart or '"ethertype_88b5"' in uart:
        return EXIT_REJECT, "HOST_COUNTS_IN_UART", info
    mspeed = LINK_SPEED.search(uart)
    if mspeed:
        info["uart_speed"] = int(mspeed.group(1))
    if require_speed is not None:
        if not mspeed or int(mspeed.group(1)) != require_speed:
            return EXIT_REJECT, f"NO_SPEED_{require_speed}", info
    et, mac = parse_host_counts(host)
    info["ethertype_88b5"] = et
    info["board_mac"] = mac
    if et < 32 or mac < 32:
        return EXIT_REJECT, "NO_HOST_32", info
    mrun = SEQ_RUN.search(host)
    seq_run = int(mrun.group(1)) if mrun else 0
    info["seq_run"] = seq_run
    if require_speed == 1000 and seq_run < 32:
        return EXIT_REJECT, "NO_SEQ_RUN_32", info
    if require_speed == 1000:
        return EXIT_ACCEPT, "WIRE_88B5_32_1G", info
    return EXIT_ACCEPT, "WIRE_88B5_32", info


def _selftest() -> None:
    uart = (
        "  [ SEALED] BootSessionRoot:     aabbccdd\n"
        "gpos> net wire txburst 32\n"
        "gem wire txburst: OK n=32 ethertype=0x88B5 "
        "dst=ff:ff:ff:ff:ff:ff src=02:00:00:47:01:50 "
        "(reuse wire_tx_raw; not PhaseNet)\n"
        "gpos>\n"
    )
    host = "[txcap] done total=40 board_mac=32 ethertype_88b5=32 seq_run=32\n"
    code, reason, info = classify(uart, host)
    assert code == EXIT_ACCEPT, (code, reason, info)
    code, reason, _info = classify(uart, "")
    assert code == EXIT_REJECT and reason == "NO_HOST_32", (code, reason)
    leak = uart + "ethertype_88b5=32\n"
    code, reason, _info = classify(leak, host)
    assert code == EXIT_REJECT and reason == "HOST_COUNTS_IN_UART", (code, reason)
    mixed = "result=PASS\n" + uart
    code, reason, _info = classify(mixed, host)
    assert code == EXIT_REJECT and reason == "HOST_RESULT_LINE", (code, reason)
    jhost = (
        '{"type":"HOST_PARSER_EVENT","message":"sniff_done",'
        '"data":{"ethertype_88b5":32,"board_mac":32}}\n'
    )
    code, reason, _info = classify(uart, jhost)
    assert code == EXIT_ACCEPT, (code, reason, _info)
    g1u = uart.replace(
        "gpos> net wire txburst 32\n",
        "gem link: phy=0 UP bmsr=0x79AD speed=1000 duplex=full\n"
        "gpos> net wire txburst 32\n",
    )
    code, reason, info = classify(g1u, host, require_speed=1000)
    assert code == EXIT_ACCEPT and reason == "WIRE_88B5_32_1G", (code, reason, info)
    code, reason, _info = classify(uart, host, require_speed=1000)
    assert code == EXIT_REJECT and reason == "NO_SPEED_1000", (code, reason)


if __name__ == "__main__":
    _selftest()
    print("a16_88b5_txburst_board selftest OK")
