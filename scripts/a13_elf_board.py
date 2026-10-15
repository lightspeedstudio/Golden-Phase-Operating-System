#!/usr/bin/env python3
"""A13 board classifier: ARM ELF32 enters the isolated user window.

Does not open COM. Does not modify kernel output.
Host fixtures only. Existing transcripts are not rewritten.
NativeIsolationUnavailable after install/run is a failure.
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

INSTALL_RE = re.compile(r"installed app=(\S+) root=([0-9a-f]{64})")
APP = "legal-elf"

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
    if "MMU: CPU0 isolation" not in text:
        return EXIT_REJECT, "NO_ISOLATION_ENABLE", {}
    inst = INSTALL_RE.search(text)
    if not inst or inst.group(1) != APP:
        return EXIT_REJECT, "NO_ELF_INSTALL", {}
    if "NativeIsolationUnavailable" in text:
        return EXIT_REJECT, "ELF_STILL_UNAVAILABLE", {}
    if "LEGAL_OK" not in text:
        return EXIT_REJECT, "NO_LEGAL_OK", {}
    if "MMUISO:" not in text or "enter" not in text:
        return EXIT_REJECT, "NO_USER_ENTER", {}
    if "MMUISO: rc=0 back" not in text:
        return EXIT_REJECT, "NO_CLEAN_EXIT", {}
    if "MMU_VIOLATION" in text:
        return EXIT_REJECT, "UNEXPECTED_VIOLATION", {}
    if "SENTINEL_APP_OUTPUT:" not in text:
        return EXIT_REJECT, "NO_LAUNCH_OUTPUT", {}
    return EXIT_ACCEPT, "ELF_A13", {
        "run_id": roots[0],
        "app": inst.group(1),
        "root": inst.group(2),
    }


def _selftest() -> None:
    boot = (
        "  [ SEALED] BootSessionRoot:     aabbccdd\n"
        "MMU: CPU0 isolation (Client D0, L2 kernel W^X, PMM priv); CPU1 kernel-only L1\n"
        "gpos> app install elf.gpa\n"
        "installed app=legal-elf root=" + ("ab" * 32) + "\n"
        "gpos> app run legal-elf\n"
        "MMUISO: l1=01340000 code=01330000 sp=01334000 enter\n"
        "LEGAL_OK\n"
        "MMUISO: rc=0 back\n"
        "SENTINEL_APP_OUTPUT:\n"
        "package=legal-elf\n"
    )
    code, reason, info = classify(boot)
    assert code == EXIT_ACCEPT, (code, reason, info)
    refused = boot.replace("LEGAL_OK\nMMUISO: rc=0 back\n", "NativeIsolationUnavailable\n")
    code, reason, _ = classify(refused)
    assert code == EXIT_REJECT and reason == "ELF_STILL_UNAVAILABLE", (code, reason)


if __name__ == "__main__":
    _selftest()
    if len(sys.argv) == 1:
        print("selftest ok")
        raise SystemExit(0)
    text = Path(sys.argv[1]).read_text(encoding="utf-8", errors="replace")
    code, reason, info = classify(text)
    print(reason, info)
    raise SystemExit(code)
