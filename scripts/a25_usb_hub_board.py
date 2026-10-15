#!/usr/bin/env python3
"""A25 classifier: hub 214b:7250 with keyboard and mouse behind it.

Does not open COM. Does not modify kernel output.
Direct J10 HID is A15, not this test.
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

_HUB_VIDPID = re.compile(r"214b:7250", re.I)
_CLASS_HUB = re.compile(r"class=HUB\b")
_KBD = re.compile(r"class=HID-keyboard|proto=1\b|KeyDown ")
_MOUSE = re.compile(r"class=HID-mouse|proto=2\b|MouseMove |MouseButton ")
_CHILD_HUB = re.compile(r"hub=\d+\s+port=\d+")
_KEYS_N = re.compile(
    r"usb keys:\s+(\d+)\s+report\(s\) captured from\s+([0-9a-fA-F]{4}):([0-9a-fA-F]{4})\s+proto=(\d+)"
)


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
    if not _HUB_VIDPID.search(text) or not _CLASS_HUB.search(text):
        return EXIT_REJECT, "NO_HUB_214B_7250", info
    if not _CHILD_HUB.search(text):
        return EXIT_REJECT, "NO_HUB_CHILD", info
    if not _KBD.search(text):
        return EXIT_REJECT, "NO_KEYBOARD_BEHIND_HUB", info
    if not _MOUSE.search(text):
        return EXIT_REJECT, "NO_MOUSE_BEHIND_HUB", info
    kbd_report = "KeyDown " in text or any(
        int(m.group(1)) > 0 and int(m.group(4)) == 1 for m in _KEYS_N.finditer(text)
    )
    mouse_report = (
        "MouseMove " in text
        or "MouseButton " in text
        or any(int(m.group(1)) > 0 and int(m.group(4)) == 2 for m in _KEYS_N.finditer(text))
    )
    info["kbd_report"] = kbd_report
    info["mouse_report"] = mouse_report
    if not kbd_report:
        return EXIT_REJECT, "NO_KEYBOARD_REPORT", info
    if not mouse_report:
        return EXIT_REJECT, "NO_MOUSE_REPORT", info
    return EXIT_ACCEPT, "USB_HUB_HID", info


def _selftest() -> None:
    uart = (
        "  [ SEALED] BootSessionRoot:     aabbccdd\n"
        "usb1: 214b:7250 class=HUB addr=1 cfg=1 ep0=64 high-speed\n"
        "usb2: 3151:4015 class=HID-keyboard addr=2 cfg=1 ep0=8 full-speed"
        " int-in=0x81 maxpkt=8 interval=10 hub=1 port=1\n"
        "usb3: 046d:c092 class=HID-mouse addr=3 cfg=1 ep0=8 full-speed"
        " int-in=0x81 maxpkt=8 interval=10 hub=1 port=2\n"
        "usb keys: 4 report(s) captured from 3151:4015 proto=1\n"
        "usb keys: 6 report(s) captured from 046d:c092 proto=2\n"
        "gpos>\n"
    )
    code, reason, info = classify(uart)
    assert code == EXIT_ACCEPT, (code, reason, info)
    no_hub = uart.replace("214b:7250 class=HUB", "0000:0000 class=non-HID")
    code, reason, _ = classify(no_hub)
    assert code == EXIT_REJECT and reason == "NO_HUB_214B_7250", (code, reason)
    mixed = "result=PASS\n" + uart
    code, reason, _ = classify(mixed)
    assert code == EXIT_REJECT and reason == "HOST_RESULT_LINE", (code, reason)


if __name__ == "__main__":
    _selftest()
    print("a25_usb_hub_board selftest OK")
