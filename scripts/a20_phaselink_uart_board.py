#!/usr/bin/env python3
"""A20 classifier: PhaseLink is GEM 0x88B5; console is UART0; no bridge.

Does not open COM. Does not modify kernel output.
Does not require a live 0x88B5 peer. Fail-closed if splash calls UART the transport.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

_SCRIPTS = Path(__file__).resolve().parent
_JOURNAL = _SCRIPTS.parent / "fpga" / "zynq" / "research" / "phasefs-persistent-journal-v1"
sys.path.insert(0, str(_JOURNAL))

from a01_raw_uart import (  # noqa: E402
    boot_session_roots,
    is_raw_uart_evidence,
)

EXIT_ACCEPT = 0
EXIT_REJECT = 1


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
    if "loopback/UART" in text:
        return EXIT_REJECT, "UART_CLAIMED_AS_PHASELINK_TRANSPORT", info
    if "transport: hosted loopback" in text and "GEM0 EtherType 0x88B5" not in text:
        return EXIT_REJECT, "HOSTED_LOOPBACK_ON_BOARD", info
    if "GEM0 EtherType 0x88B5" not in text:
        return EXIT_REJECT, "NO_GEM_ETHERTYPE", info
    if "PS UART0" not in text:
        return EXIT_REJECT, "NO_UART_CONSOLE", info
    if "UART-GEM bridge:          none" not in text and "bridge: none" not in text:
        return EXIT_REJECT, "NO_BRIDGE_NONE", info
    return EXIT_ACCEPT, "PHASELINK_UART_A20", info


def _selftest() -> None:
    uart = (
        "  [ SEALED] BootSessionRoot:     aabbccdd\n"
        "  [ LINK   ] transport:                GEM0 EtherType 0x88B5\n"
        "  [ LINK   ] console:                  PS UART0\n"
        "  [ LINK   ] UART-GEM bridge:          none\n"
        "gpos> phaselink status\n"
        "transport: GEM0 EtherType 0x88B5\n"
        "console: PS UART0\n"
        "bridge: none\n"
        "gpos>\n"
    )
    code, reason, info = classify(uart)
    assert code == EXIT_ACCEPT, (code, reason, info)
    stale = uart.replace("GEM0 EtherType 0x88B5", "loopback/UART")
    code, reason, _info = classify(stale)
    assert code == EXIT_REJECT and reason == "UART_CLAIMED_AS_PHASELINK_TRANSPORT", (
        code,
        reason,
    )


if __name__ == "__main__":
    _selftest()
    print("a20_phaselink_uart_board selftest OK")
