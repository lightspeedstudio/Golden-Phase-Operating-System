#!/usr/bin/env python3
"""A19 classifier: File-cap gated write/cat/mkdir/rm on one PZ7020 boot.

Does not open COM. Does not modify kernel output.
Host fixtures only. Fail-closed without a cap remains HOST_UNIT_TEST.
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

_OPENS = re.compile(r"^opens:\s+(\d+)\s*$", re.M)
_AUTH_OK = re.compile(r"^authorize_ok:\s+(\d+)\s*$", re.M)
_LIVE_CAPS = re.compile(r"^live_capabilities:\s+(\d+)\s*$", re.M)


def _forbidden(text: str) -> str | None:
    if re.search(r"^result=PASS\s*$", text, flags=re.M):
        return "HOST_RESULT_LINE"
    if re.search(r"^===== .+ (PASS|FAIL)", text, flags=re.M):
        return "HOST_STEP_BANNER"
    return None


def _last_int(pat: re.Pattern[str], text: str) -> int | None:
    hits = [int(m.group(1)) for m in pat.finditer(text)]
    return hits[-1] if hits else None


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
    if "captable unavailable" in text:
        return EXIT_REJECT, "CAPTABLE_FEATURE_OFF", info
    if "fs_open is a no-op stub" in text:
        return EXIT_REJECT, "FS_OPEN_STUB", info
    if "wrote 15 bytes to /a19.txt" not in text:
        return EXIT_REJECT, "NO_WRITE_FILE", info
    if "a19-file-cap-ok" not in text:
        return EXIT_REJECT, "NO_CAT_FILE", info
    if "wrote 9 bytes to /a19dir/nested.txt" not in text:
        return EXIT_REJECT, "NO_MKDIR_PARENT_WRITE", info
    if "nested-ok" not in text:
        return EXIT_REJECT, "NO_CAT_NESTED", info
    if "rm /a19.txt" not in text:
        return EXIT_REJECT, "NO_RM", info
    if "cat /a19.txt" not in text:
        return EXIT_REJECT, "NO_CAT", info
    if "filesystem:" not in text or "not found" not in text.lower():
        return EXIT_REJECT, "NO_CAT_AFTER_RM_NOT_FOUND", info
    if "error: captable" in text or "captable: invalid" in text:
        return EXIT_REJECT, "CAPTABLE_ERROR_ON_SHELL_PATH", info

    opens = _last_int(_OPENS, text)
    auth_ok = _last_int(_AUTH_OK, text)
    live_caps = _last_int(_LIVE_CAPS, text)
    info.update({"opens": opens, "authorize_ok": auth_ok, "live_capabilities": live_caps})
    if opens is None or opens < 6:
        return EXIT_REJECT, "OPENS_NOT_INCREASED", info
    if auth_ok is None or auth_ok < 6:
        return EXIT_REJECT, "AUTHORIZE_OK_NOT_INCREASED", info
    if live_caps is None or live_caps != 0:
        return EXIT_REJECT, "LIVE_CAPS_NOT_DROPPED", info

    return EXIT_ACCEPT, "CAPTABLE_FS_A19", info


def _selftest() -> None:
    uart = (
        "  [ SEALED] BootSessionRoot:     aabbccdd\n"
        "gpos> captable\n"
        "opens: 0\n"
        "authorize_ok: 0\n"
        "live_capabilities: 0\n"
        "gpos> write /a19.txt a19-file-cap-ok\n"
        "wrote 15 bytes to /a19.txt\n"
        "gpos> cat /a19.txt\n"
        "a19-file-cap-ok\n"
        "gpos> mkdir /a19dir\n"
        "gpos> write /a19dir/nested.txt nested-ok\n"
        "wrote 9 bytes to /a19dir/nested.txt\n"
        "gpos> cat /a19dir/nested.txt\n"
        "nested-ok\n"
        "gpos> rm /a19.txt\n"
        "gpos> cat /a19.txt\n"
        "filesystem: not found: /a19.txt\n"
        "gpos> captable\n"
        "opens: 6\n"
        "authorize_ok: 6\n"
        "live_capabilities: 0\n"
        "gpos>\n"
    )
    code, reason, info = classify(uart)
    assert code == EXIT_ACCEPT, (code, reason, info)
    mixed = "result=PASS\n" + uart
    code, reason, _info = classify(mixed)
    assert code == EXIT_REJECT and reason == "HOST_RESULT_LINE", (code, reason)


if __name__ == "__main__":
    _selftest()
    print("a19_captable_fs_board selftest OK")
