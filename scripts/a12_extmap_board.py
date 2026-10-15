#!/usr/bin/env python3
"""A12 board classifier: recovered PhaseFs files have extmap extents.

Does not open COM. Does not modify kernel output.
Host fixtures only. Existing transcripts are not rewritten.
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

A12_PATH = "/a12.bin"
A12_PAYLOAD = "a12-extent"
LIVE_RE = re.compile(r"blocks_live:\s+(\d+)")

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


def _live_blocks(text: str) -> list[int]:
    return [int(m.group(1)) for m in LIVE_RE.finditer(text)]


def classify_install(text: str) -> tuple[int, str, dict]:
    raw_ok, raw_reason = is_raw_uart_evidence(text)
    if not raw_ok:
        return EXIT_REJECT, raw_reason, {}
    bad = _forbidden(text)
    if bad:
        return EXIT_REJECT, bad, {}
    roots = boot_session_roots(text)
    if len(roots) != 1:
        return EXIT_REJECT, "INSTALL_BOOT_EPOCH", {"roots": roots}
    if "RAM PhaseFs only" in text or "no partition 1" in text:
        return EXIT_REJECT, "RAM_FALLBACK_BOOT", {}
    if "backend: SD_PARTITION_1" not in text or "durable: YES" not in text:
        return EXIT_REJECT, "WAL_NOT_DURABLE", {}
    check_ok, check_reason = board_check_durable_in_raw(text)
    if not check_ok:
        return EXIT_REJECT, check_reason, {}
    live = _live_blocks(text)
    if len(live) < 2:
        return EXIT_REJECT, "NO_EXTMAP_PAIR", {}
    if live[-1] <= live[0]:
        return EXIT_REJECT, "EXTMAP_DID_NOT_GROW", {"live": live}
    if A12_PAYLOAD not in text:
        return EXIT_REJECT, "PAYLOAD_MISSING", {}
    info = {"run_id": roots[0], "blocks_live": live[-1], "payload": A12_PAYLOAD}
    return EXIT_ACCEPT, "INSTALL_EXTMAP", info


def classify_recover(text: str, install_info: dict) -> tuple[int, str]:
    raw_ok, raw_reason = is_raw_uart_evidence(text)
    if not raw_ok:
        return EXIT_REJECT, raw_reason
    bad = _forbidden(text)
    if bad:
        return EXIT_REJECT, bad
    roots = boot_session_roots(text)
    if len(roots) != 1:
        return EXIT_REJECT, "RECOVER_BOOT_EPOCH"
    if "RAM PhaseFs only" in text or "no partition 1" in text:
        return EXIT_REJECT, "RAM_FALLBACK_RECOVER"
    check_ok, check_reason = board_check_durable_in_raw(text)
    if not check_ok:
        return EXIT_REJECT, check_reason
    live = _live_blocks(text)
    if not live or live[-1] < 1:
        return EXIT_REJECT, "EXTENTS_EMPTY_AFTER_RESET"
    if live[-1] < install_info["blocks_live"]:
        return EXIT_REJECT, "EXTENTS_SHRANK_AFTER_RESET"
    if install_info["payload"] not in text:
        return EXIT_REJECT, "PAYLOAD_MISSING_AFTER_RESET"
    return EXIT_ACCEPT, "RECOVER_EXTMAP"


def _selftest() -> None:
    install = (
        "  [ SEALED] BootSessionRoot:     aa11bb22\n"
        "gpos> fsj status\n"
        "backend: SD_PARTITION_1\n"
        "durable: YES\n"
        "gpos> fsj check\n"
        "fsj check: PASS\n"
        "DURABLE: YES\n"
        "gpos> extmap\n"
        "blocks_live: 4\n"
        "gpos> write /a12.bin "
        + A12_PAYLOAD
        + "\n"
        "wrote 10 bytes to /a12.bin\n"
        "gpos> extmap\n"
        "blocks_live: 5\n"
        "gpos> cat /a12.bin\n"
        + A12_PAYLOAD
        + "\n"
        "gpos>\n"
    )
    code, reason, info = classify_install(install)
    assert code == EXIT_ACCEPT, (code, reason, info)
    recover = (
        "  [ SEALED] BootSessionRoot:     cc33dd44\n"
        "gpos> cat /a12.bin\n"
        + A12_PAYLOAD
        + "\n"
        "gpos> extmap\n"
        "blocks_live: 5\n"
        "gpos> fsj check\n"
        "fsj check: PASS\n"
        "DURABLE: YES\n"
        "gpos>\n"
    )
    code, reason = classify_recover(recover, info)
    assert code == EXIT_ACCEPT, (code, reason)
    empty = recover.replace("blocks_live: 5", "blocks_live: 0")
    code, reason = classify_recover(empty, info)
    assert code == EXIT_REJECT and reason == "EXTENTS_EMPTY_AFTER_RESET", (code, reason)
    print("a12_extmap_board selftest: ok")


def main(argv: list[str]) -> int:
    if len(argv) >= 2 and argv[1] == "--selftest":
        _selftest()
        return 0
    print("usage: a12_extmap_board.py --selftest", file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
