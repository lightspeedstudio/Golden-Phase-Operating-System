#!/usr/bin/env python3
"""A15 HID classifier: one UART must show keyboard AND mouse.

Does not open COM. Does not modify kernel output.
Does not accept keyboard reports as mouse. usb0_53 count is not required.
"""

from __future__ import annotations

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

_KEYS_N = re.compile(r"usb keys:\s+(\d+)\s+report\(s\) captured")


def _forbidden(text: str) -> str | None:
    if re.search(r"^result=PASS\s*$", text, flags=re.M):
        return "HOST_RESULT_LINE"
    if re.search(r"^===== .+ (PASS|FAIL)", text, flags=re.M):
        return "HOST_STEP_BANNER"
    return None


def _keyboard(text: str) -> bool:
    if "KeyDown " in text:
        return True
    if "keyboard attached" in text:
        return True
    if "HID-keyboard" in text:
        return True
    if re.search(r"proto=1\b", text):
        for m in _KEYS_N.finditer(text):
            if int(m.group(1)) > 0:
                return True
    return False


def _mouse(text: str) -> bool:
    if "MouseMove " in text:
        return True
    if "MouseButton " in text:
        return True
    if "HID-mouse" in text:
        return True
    if re.search(r"proto=2\b", text):
        return True
    return False


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
    kbd = _keyboard(text)
    mouse = _mouse(text)
    info["keyboard"] = kbd
    info["mouse"] = mouse
    if not kbd:
        return EXIT_REJECT, "NO_KEYBOARD", info
    if not mouse:
        return EXIT_REJECT, "NO_MOUSE", info
    return EXIT_ACCEPT, "HID_KBD_MOUSE", info


def _selftest() -> None:
    uart = (
        "  [ SEALED] BootSessionRoot:     aabbccdd\n"
        "gpos> usb hid\n"
        "usb hid: 3151:4015 addr=1 cfg=1 ifaces=2\n"
        "gpos> usb events\n"
        "usb events:\n"
        "  KeyDown usage=0x0004 mod=0x00\n"
        "  MouseMove dx=3 dy=-1\n"
        "usb events: 2 from controller\n"
        "gpos>\n"
    )
    code, reason, info = classify(uart)
    assert code == EXIT_ACCEPT, (code, reason, info)
    kbd_only = uart.replace("  MouseMove dx=3 dy=-1\n", "")
    code, reason, _info = classify(kbd_only)
    assert code == EXIT_REJECT and reason == "NO_MOUSE", (code, reason)
    mouse_only = uart.replace("  KeyDown usage=0x0004 mod=0x00\n", "")
    code, reason, _info = classify(mouse_only)
    assert code == EXIT_REJECT and reason == "NO_KEYBOARD", (code, reason)
    mixed = "result=PASS\n" + uart
    code, reason, _info = classify(mixed)
    assert code == EXIT_REJECT and reason == "HOST_RESULT_LINE", (code, reason)
    keys = (
        "  [ SEALED] BootSessionRoot:     aabbccdd\n"
        "usb1: 3151:4015 class=HID-keyboard\n"
        "usb keys: 24 report(s) captured\n"
        "  MouseButton b=1 pressed=true\n"
        "gpos>\n"
    )
    code, reason, _info = classify(keys)
    assert code == EXIT_ACCEPT, (code, reason, _info)
    mouse_keys = (
        "  [ SEALED] BootSessionRoot:     aabbccdd\n"
        "usb1: 046d:c092 class=HID-mouse\n"
        "    ep=0x81 iface=0 proto=2  <- polling\n"
        "usb keys: 8 report(s) captured\n"
        "gpos>\n"
    )
    code, reason, _info = classify(mouse_keys)
    assert code == EXIT_REJECT and reason == "NO_KEYBOARD", (code, reason)
    zero = keys.replace("usb keys: 24 report(s) captured\n", "usb keys: 0 report(s) captured\n")
    code, reason, _info = classify(zero)
    assert code == EXIT_ACCEPT, (code, reason, _info)


if __name__ == "__main__":
    _selftest()
    print("a15_hid_kbd_mouse_board selftest OK")
