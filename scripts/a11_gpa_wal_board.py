#!/usr/bin/env python3
"""A11 board classifier: GPA store files in SD WAL, not RAM txlog.

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

INSTALL_RE = re.compile(r"installed app=(\S+) root=([0-9a-f]{64})")
COMMITTED_RE = re.compile(r"committed:\s+(\d+)")
STORE_FILES = ("manifest.phase", "hooks.phase", "code.phase", "sentinel.psm1")

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


def _txlog_committed(text: str) -> list[int]:
    return [int(m.group(1)) for m in COMMITTED_RE.finditer(text)]


def _checksum_line(text: str, path: str) -> str | None:
    # Command echo then Debug-format hash_payload bytes, e.g. [10, 32, ...]
    pat = re.compile(
        r"checksum\s+" + re.escape(path) + r"\s*\r?\n(\[[0-9a-fA-Fx, ]+\])",
        re.M,
    )
    m = pat.search(text)
    return m.group(1) if m else None


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
    inst = INSTALL_RE.search(text)
    if not inst:
        return EXIT_REJECT, "NO_INSTALL", {}
    root = inst.group(2)
    store = f"/phase/packages/{root}"
    for name in STORE_FILES:
        if name not in text:
            return EXIT_REJECT, f"MISSING_{name}", {"root": root}
        if _checksum_line(text, f"{store}/{name}") is None:
            return EXIT_REJECT, f"NO_CHECKSUM_{name}", {"root": root}
    committed = _txlog_committed(text)
    if len(committed) < 2:
        return EXIT_REJECT, "NO_TXLOG_PAIR", {}
    if committed[0] != committed[-1]:
        return EXIT_REJECT, "TXLOG_COMMITTED_CHANGED", {"committed": committed}
    info = {
        "root": root,
        "app": inst.group(1),
        "run_id": roots[0],
        "committed": committed[0],
        "checksums": {
            name: _checksum_line(text, f"{store}/{name}") for name in STORE_FILES
        },
    }
    return EXIT_ACCEPT, "INSTALL_WAL", info


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
    root = install_info["root"]
    store = f"/phase/packages/{root}"
    if store not in text:
        return EXIT_REJECT, "STORE_MISSING_AFTER_RESET"
    for name, expected in install_info["checksums"].items():
        got = _checksum_line(text, f"{store}/{name}")
        if got is None:
            return EXIT_REJECT, f"NO_CHECKSUM_AFTER_RESET_{name}"
        if got != expected:
            return EXIT_REJECT, f"HASH_MISMATCH_{name}"
    return EXIT_ACCEPT, "RECOVER_WAL"


def _selftest() -> None:
    install = (
        "  [ SEALED] BootSessionRoot:     aa11bb22\n"
        "fsj: start_lba=15706112 sectors=16384\n"
        "gpos> fsj status\n"
        "backend: SD_PARTITION_1\n"
        "durable: YES\n"
        "gpos> fsj check\n"
        "fsj check: PASS\n"
        "DURABLE: YES\n"
        "gpos> txlog\n"
        "txlog\n"
        "records: 0\n"
        "committed: 0\n"
        "gpos> app install hello.gpa\n"
        "installed app=hello-a7f3c91e root="
        + ("ab" * 32)
        + "\n"
        "gpos> ls /phase/packages/"
        + ("ab" * 32)
        + "\n"
        "code.phase\nhooks.phase\nmanifest.phase\nsentinel.psm1\n"
        "gpos> checksum /phase/packages/"
        + ("ab" * 32)
        + "/manifest.phase\n"
        "[1, 2, 3, 4]\n"
        "gpos> checksum /phase/packages/"
        + ("ab" * 32)
        + "/hooks.phase\n"
        "[5, 6]\n"
        "gpos> checksum /phase/packages/"
        + ("ab" * 32)
        + "/code.phase\n"
        "[7, 8, 9]\n"
        "gpos> checksum /phase/packages/"
        + ("ab" * 32)
        + "/sentinel.psm1\n"
        "[10, 11]\n"
        "gpos> txlog\n"
        "committed: 0\n"
        "gpos>\n"
    )
    code, reason, info = classify_install(install)
    assert code == EXIT_ACCEPT, (code, reason, info)
    recover = (
        "  [ SEALED] BootSessionRoot:     cc33dd44\n"
        "fsj: recovered class=CLEAN replayed=0 discarded=0 gen=3 txn=0\n"
        "gpos> ls /phase/packages/"
        + ("ab" * 32)
        + "\n"
        "code.phase\nhooks.phase\nmanifest.phase\nsentinel.psm1\n"
        "gpos> checksum /phase/packages/"
        + ("ab" * 32)
        + "/manifest.phase\n"
        "[1, 2, 3, 4]\n"
        "gpos> checksum /phase/packages/"
        + ("ab" * 32)
        + "/hooks.phase\n"
        "[5, 6]\n"
        "gpos> checksum /phase/packages/"
        + ("ab" * 32)
        + "/code.phase\n"
        "[7, 8, 9]\n"
        "gpos> checksum /phase/packages/"
        + ("ab" * 32)
        + "/sentinel.psm1\n"
        "[10, 11]\n"
        "gpos> fsj check\n"
        "fsj check: PASS\n"
        "DURABLE: YES\n"
        "gpos>\n"
    )
    code, reason = classify_recover(recover, info)
    assert code == EXIT_ACCEPT, (code, reason)

    mixed = "result=PASS\n" + install
    ok, _ = is_raw_uart_evidence(mixed)
    assert not ok
    code, reason, _ = classify_install(mixed)
    assert code == EXIT_REJECT

    txlog_used = install.replace("committed: 0\n", "committed: 1\n", 1)
    # first committed 0, second still 0 in original; bump the first only via replace first
    # The replace above changes the FIRST occurrence, so pair is 1 then 0 → reject.
    code, reason, _ = classify_install(txlog_used)
    assert code == EXIT_REJECT and reason == "TXLOG_COMMITTED_CHANGED", (code, reason)

    print("a11_gpa_wal_board selftest: ok")


def main(argv: list[str]) -> int:
    if len(argv) >= 2 and argv[1] == "--selftest":
        _selftest()
        return 0
    print(
        "usage: a11_gpa_wal_board.py --selftest",
        file=sys.stderr,
    )
    return 2


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
