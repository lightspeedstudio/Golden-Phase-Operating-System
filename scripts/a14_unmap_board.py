#!/usr/bin/env python3
"""A14 board classifier: Sentinel contain unmaps the user TTBR window.

Does not open COM. Does not modify kernel output.
Host fixtures only. Existing transcripts are not rewritten.
Kill/caps without MMUISO unmap is a failure.
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

UNMAP_RE = re.compile(r"MMUISO: unmap pid=(\d+) asid=(\d+) l1=([0-9a-f]{8})")
PAYLOAD = "kernel-write"

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
    um = UNMAP_RE.search(text)
    if not um:
        return EXIT_REJECT, "NO_TTBR_UNMAP", {}
    if "MMU_VIOLATION: ISOLATED" not in text:
        return EXIT_REJECT, "NO_CONTAIN", {}
    if "LAST_UNMAP: YES" not in text:
        return EXIT_REJECT, "NO_UNMAP_STATUS", {}
    if f"{PAYLOAD}: PASS" not in text:
        return EXIT_REJECT, "NO_KERNEL_WRITE_PASS", {}
    return EXIT_ACCEPT, "UNMAP_A14", {
        "run_id": roots[0],
        "pid": um.group(1),
        "asid": um.group(2),
        "l1": um.group(3),
    }


def _selftest() -> None:
    boot = (
        "  [ SEALED] BootSessionRoot:     aabbccdd\n"
        "MMU: CPU0 isolation (Client D0, L2 kernel W^X, PMM priv); CPU1 kernel-only L1\n"
        "gpos> mmuiso kernel-write\n"
        "MMUISO: l1=1fff8000 code=1fff1000 sp=1fff5000 enter\n"
        "MMU abort class=Permission far=00100000 fsr=00000d0d pc=1fff1000\n"
        "MMUISO: rc=2 back\n"
        "MMUISO: unmap pid=1 asid=3 l1=1fff8000\n"
        "MMU_VIOLATION: ISOLATED\n"
        "kernel-write: PASS\n"
        "gpos> mmuiso status\n"
        "LAST_UNMAP: YES\n"
        "gpos>\n"
    )
    code, reason, info = classify(boot)
    assert code == EXIT_ACCEPT, (code, reason, info)
    missing = boot.replace("MMUISO: unmap pid=1 asid=3 l1=1fff8000\n", "")
    code, reason, _ = classify(missing)
    assert code == EXIT_REJECT and reason == "NO_TTBR_UNMAP", (code, reason)


if __name__ == "__main__":
    _selftest()
    if len(sys.argv) == 1:
        print("selftest ok")
        raise SystemExit(0)
    text = Path(sys.argv[1]).read_text(encoding="utf-8", errors="replace")
    code, reason, info = classify(text)
    print(reason, info)
    raise SystemExit(code)
