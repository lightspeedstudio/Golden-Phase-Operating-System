#!/usr/bin/env python3
"""A15 board classifier: USB0 SPI 53 is a first-class irqreg slot.

Does not open COM. Does not modify kernel output.
Host fixtures only. Existing transcripts are not rewritten.
irqreg without usb0_53 is a failure. HID report count>0 is not required.
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

USB0_ROW = re.compile(r"usb0_53:\s+count=\d+\s+storm_masked=(true|false)")
G1P_ROW = re.compile(r"g1p_irq_61:\s+count=\d+")
GEM_ROW = re.compile(r"gem0_rx_54:\s+count=\d+")
TIMER_ROW = re.compile(r"priv_timer_29:\s+count=\d+")

EXIT_ACCEPT = 0
EXIT_REJECT = 1


def _forbidden(text: str) -> str | None:
    if "journal i/o" in text:
        return "JOURNAL_IO"
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
        return EXIT_REJECT, "BOOT_EPOCH", {}
    if not G1P_ROW.search(text):
        return EXIT_REJECT, "NO_G1P_ROW", {}
    if not GEM_ROW.search(text):
        return EXIT_REJECT, "NO_GEM_ROW", {}
    if not TIMER_ROW.search(text):
        return EXIT_REJECT, "NO_TIMER_ROW", {}
    um = USB0_ROW.search(text)
    if not um:
        return EXIT_REJECT, "NO_USB0_53", {}
    if "usb0: host up @ 0xE0002000" not in text:
        return EXIT_REJECT, "NO_USB_PROBE", {}
    return EXIT_ACCEPT, "USB_A15", {
        "run_id": roots[0],
        "usb0_storm": um.group(1),
    }


def _selftest() -> None:
    boot = (
        "  [ SEALED] BootSessionRoot:     aabbccdd\n"
        "gpos> irqreg\n"
        "irqreg\n"
        "             g1p_irq_61:   count=0 storm_masked=false\n"
        "             gem0_rx_54:   count=0 storm_masked=false\n"
        "             usb0_53:      count=0 storm_masked=false\n"
        "             priv_timer_29: count=12 storm_masked=false\n"
        "gpos> usb probe\n"
        "usb0: host up @ 0xE0002000 (ULPI) -- port powered, no device\n"
        "gpos>\n"
    )
    code, reason, info = classify(boot)
    assert code == EXIT_ACCEPT, (code, reason, info)
    missing = boot.replace("             usb0_53:      count=0 storm_masked=false\n", "")
    code, reason, _ = classify(missing)
    assert code == EXIT_REJECT and reason == "NO_USB0_53", (code, reason)


if __name__ == "__main__":
    _selftest()
    if len(sys.argv) == 1:
        print("selftest ok")
        raise SystemExit(0)
    text = Path(sys.argv[1]).read_text(encoding="utf-8", errors="replace")
    code, reason, info = classify(text)
    print(reason, info)
    raise SystemExit(code)
