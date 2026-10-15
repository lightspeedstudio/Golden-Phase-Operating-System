#!/usr/bin/env python3
"""A17 board classifier: identity snapshot is a WAL redo on p1, not FAT p0.

Does not open COM. Does not modify kernel output.
Host fixtures only. Existing transcripts are not rewritten.
passwd secrets must not appear in UART. /idstore.bin GPID + checksum match across RESET.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

_SCRIPTS = Path(__file__).resolve().parent
_JOURNAL = _SCRIPTS.parent / "fpga" / "zynq" / "research" / "phasefs-persistent-journal-v1"
sys.path.insert(0, str(_JOURNAL))

from a01_raw_uart import (  # noqa: E402
    board_check_durable_in_raw,
    boot_session_roots,
    is_raw_uart_evidence,
)

GPID_HEX = re.compile(r"47 50 49 44", re.I)
CHECKSUM = re.compile(
    r"checksum\s+/idstore\.bin\s*\r?\n(\[[0-9a-fA-Fx, ]+\])",
    re.M,
)
ACTIVE = re.compile(r"root credential:\s+active")
FSJ_RECOVER = re.compile(
    r"fsj: recovered class=(\S+) replayed=(\d+) discarded=(\d+) gen=(\d+)"
)

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


def _checksum(text: str) -> str | None:
    m = CHECKSUM.search(text)
    return m.group(1) if m else None


def classify_persist(text: str) -> tuple[int, str, dict]:
    bad = _forbidden(text)
    if bad:
        return EXIT_REJECT, bad, {}
    ok, reason = is_raw_uart_evidence(text)
    if not ok:
        return EXIT_REJECT, reason, {}
    roots = boot_session_roots(text)
    if len(roots) != 1:
        return EXIT_REJECT, "PERSIST_BOOT_EPOCH", {"roots": roots}
    if "RAM PhaseFs only" in text or "no partition 1" in text:
        return EXIT_REJECT, "RAM_FALLBACK_BOOT", {}
    check_ok, check_reason = board_check_durable_in_raw(text)
    if not check_ok:
        return EXIT_REJECT, check_reason, {}
    if not GPID_HEX.search(text):
        return EXIT_REJECT, "NO_GPID", {}
    csum = _checksum(text)
    if not csum:
        return EXIT_REJECT, "NO_IDSTORE_CHECKSUM", {}
    if not ACTIVE.search(text) and "root password set" not in text:
        return EXIT_REJECT, "ROOT_NOT_ACTIVE", {}
    rec = FSJ_RECOVER.search(text)
    return EXIT_ACCEPT, "IDENTITY_WAL_PERSIST", {
        "run_id": roots[0],
        "checksum": csum,
        "fsj": rec.group(0) if rec else "",
    }


def classify_recover(text: str, persist_info: dict) -> tuple[int, str]:
    bad = _forbidden(text)
    if bad:
        return EXIT_REJECT, bad
    ok, reason = is_raw_uart_evidence(text)
    if not ok:
        return EXIT_REJECT, reason
    roots = boot_session_roots(text)
    if len(roots) != 1:
        return EXIT_REJECT, "RECOVER_BOOT_EPOCH"
    if "REJECTED_CORRUPT" in text:
        return EXIT_REJECT, "LEFTOVER_CORRUPT"
    rec = FSJ_RECOVER.search(text)
    if not rec:
        return EXIT_REJECT, "NO_FSJ_RECOVER"
    if rec.group(0) == persist_info.get("fsj"):
        return EXIT_REJECT, "SAME_FSJ_SPLASH"
    if rec.group(1) != "CLEAN":
        return EXIT_REJECT, "RECOVER_NOT_CLEAN"
    if rec.group(3) != "0":
        return EXIT_REJECT, "RECOVER_DISCARDED"
    check_ok, check_reason = board_check_durable_in_raw(text)
    if not check_ok:
        return EXIT_REJECT, check_reason
    if not GPID_HEX.search(text):
        return EXIT_REJECT, "NO_GPID"
    csum = _checksum(text)
    if not csum:
        return EXIT_REJECT, "NO_IDSTORE_CHECKSUM"
    if csum != persist_info.get("checksum"):
        return EXIT_REJECT, "CHECKSUM_MISMATCH"
    if not ACTIVE.search(text):
        return EXIT_REJECT, "ROOT_NOT_ACTIVE"
    return EXIT_ACCEPT, "IDENTITY_A17"


def _selftest() -> None:
    persist = (
        "  [ SEALED] BootSessionRoot:     aabbccdd\n"
        "fsj: recovered class=CLEAN replayed=0 discarded=0 gen=1 txn=0\n"
        "gpos> fsj check\n"
        "fsj check: PASS\n"
        "gpos> hexdump /idstore.bin --length 8\n"
        "00000000  47 50 49 44 01 00 00 00  |GPID....|\n"
        "gpos> checksum /idstore.bin\n"
        "[1, 2, 3]\n"
        "gpos> passwd --status\n"
        "root credential: active\n"
        "gpos>\n"
    )
    code, reason, info = classify_persist(persist)
    assert code == EXIT_ACCEPT, (code, reason, info)
    recover = (
        "  [ SEALED] BootSessionRoot:     11223344\n"
        "fsj: recovered class=CLEAN replayed=0 discarded=0 gen=2 txn=0\n"
        "gpos> fsj check\n"
        "fsj check: PASS\n"
        "gpos> hexdump /idstore.bin --length 8\n"
        "00000000  47 50 49 44 01 00 00 00  |GPID....|\n"
        "gpos> checksum /idstore.bin\n"
        "[1, 2, 3]\n"
        "gpos> passwd --status\n"
        "root credential: active\n"
        "gpos>\n"
    )
    code, reason = classify_recover(recover, info)
    assert code == EXIT_ACCEPT, (code, reason)
    bad = recover.replace("[1, 2, 3]", "[9, 9, 9]")
    code, reason = classify_recover(bad, info)
    assert code == EXIT_REJECT and reason == "CHECKSUM_MISMATCH", (code, reason)
    leftover = recover.replace(
        "class=CLEAN replayed=0 discarded=0",
        "class=REJECTED_CORRUPT replayed=0 discarded=1",
    )
    code, reason = classify_recover(leftover, info)
    assert code == EXIT_REJECT and reason == "LEFTOVER_CORRUPT", (code, reason)
    same_root = recover.replace("11223344", "aabbccdd")
    code, reason = classify_recover(same_root, info)
    assert code == EXIT_ACCEPT, (code, reason)
    same_splash = same_root.replace("gen=2", "gen=1")
    code, reason = classify_recover(same_splash, info)
    assert code == EXIT_REJECT and reason == "SAME_FSJ_SPLASH", (code, reason)


if __name__ == "__main__":
    _selftest()
    if len(sys.argv) == 1:
        print("selftest ok")
        raise SystemExit(0)
    text = Path(sys.argv[1]).read_text(encoding="utf-8", errors="replace")
    code, reason, info = classify_persist(text)
    print(reason, info)
    raise SystemExit(code)
