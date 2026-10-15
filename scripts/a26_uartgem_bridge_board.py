#!/usr/bin/env python3
"""A26 classifier: UART-GEM 0x88B5 round-trip + oversize refuse.

Does not open COM. Does not modify kernel output.
Peer sidecar is host evidence; UART must show board rx of ack.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

_SCRIPTS = Path(__file__).resolve().parent
_JOURNAL = _SCRIPTS.parent / "fpga" / "zynq" / "research" / "phasefs-persistent-journal-v1"
sys.path.insert(0, str(_SCRIPTS))
sys.path.insert(0, str(_JOURNAL))

from a01_raw_uart import boot_session_roots, is_raw_uart_evidence  # noqa: E402

EXIT_ACCEPT = 0
EXIT_REJECT = 1

_ACK = re.compile(r"bridge rx:\s+ack:ping-e2e")
_ETH = re.compile(r"ethertype=0x88B5")
_TOO_LONG = "bridge: uart payload exceeds bound"
_LABEL = "bridge: uart-gem"


def _forbidden(text: str) -> str | None:
    if re.search(r"^result=PASS\s*$", text, flags=re.M):
        return "HOST_RESULT_LINE"
    if re.search(r"^===== .+ (PASS|FAIL)", text, flags=re.M):
        return "HOST_STEP_BANNER"
    return None


def classify(text: str) -> tuple[int, str, dict]:
    bad = _forbidden(text)
    if bad:
        return EXIT_REJECT, bad, {}
    ok, reason = is_raw_uart_evidence(text)
    if not ok:
        return EXIT_REJECT, reason, {}
    roots = boot_session_roots(text)
    if len(roots) != 1:
        return EXIT_REJECT, "BOOT_EPOCH", {"roots": roots}
    info = {"run_id": roots[0]}
    if not _ETH.search(text) or _LABEL not in text:
        return EXIT_REJECT, "NO_BRIDGE_TX", info
    if not _ACK.search(text):
        return EXIT_REJECT, "NO_PEER_ACK", info
    if _TOO_LONG not in text:
        return EXIT_REJECT, "NO_OVERSIZE_REFUSE", info
    return EXIT_ACCEPT, "UART_GEM_BRIDGE", info


def _selftest() -> None:
    uart = (
        "  [ SEALED] BootSessionRoot:     aabbccdd\n"
        "phaselink initialized: board -> peer peer\n"
        "bridge tx: 200 bytes ethertype=0x88B5\n"
        "bridge rx: ack:ping-e2e\n"
        "bridge: uart-gem\n"
        "bridge: uart payload exceeds bound\n"
        "gpos>\n"
    )
    code, reason, info = classify(uart)
    assert code == EXIT_ACCEPT, (code, reason, info)
    code, reason, _ = classify(uart.replace("ack:ping-e2e", "nope"))
    assert code == EXIT_REJECT and reason == "NO_PEER_ACK", (code, reason)
    mixed = "result=PASS\n" + uart
    code, reason, _ = classify(mixed)
    assert code == EXIT_REJECT and reason == "HOST_RESULT_LINE", (code, reason)


if __name__ == "__main__":
    _selftest()
    print("a26_uartgem_bridge_board selftest OK")
