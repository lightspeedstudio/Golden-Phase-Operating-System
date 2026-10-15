#!/usr/bin/env python3
"""A10: persist must not claim txlog and SD WAL are one journal.

Does not format SD. Does not talk to the board.
"""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TXLOG = ROOT / "src" / "kernel" / "txlog.rs"
KERNEL = ROOT / "src" / "kernel" / "kernel.rs"
JOURNAL = ROOT / "src" / "fs" / "journal" / "mod.rs"
SYSCALL = ROOT / "src" / "kernel" / "syscall.rs"

OLD_TXLOG = (
    "kernel::syscall`'s `FS_WRITE_FILE`/`FS_MKDIR`/`FS_RM` handlers route"
)
OLD_KERNEL = "FS_WRITE_FILE/FS_MKDIR/FS_RM handlers route each single-op mutation"
HELPER = "wal_unavailable_falls_back_to_txlog"


def claims_always_routes_txlog(src: str) -> bool:
    return OLD_TXLOG in src or OLD_KERNEL in src


def _selftest() -> None:
    assert claims_always_routes_txlog(OLD_TXLOG) is True
    assert claims_always_routes_txlog(OLD_KERNEL) is True
    honest = (
        "Persist uses this only when phasefs-journal is off, or when board "
        "WAL cannot start; WAL success skips txlog."
    )
    assert claims_always_routes_txlog(honest) is False

    txlog = TXLOG.read_text(encoding="utf-8")
    kernel = KERNEL.read_text(encoding="utf-8")
    journal = JOURNAL.read_text(encoding="utf-8")
    syscall = SYSCALL.read_text(encoding="utf-8")
    assert not claims_always_routes_txlog(txlog), (
        "A10: txlog.rs still says persist always routes through txlog"
    )
    assert not claims_always_routes_txlog(kernel), (
        "A10: kernel.rs still says FS handlers always route through txlog"
    )
    assert "skips `txlog` when WAL succeeds" in journal
    assert HELPER in journal
    assert HELPER in syscall
    assert "WAL success does not call this" in syscall
    print("a10_txlog_journal_docs selftest: ok")


def main(argv: list[str]) -> int:
    if len(argv) >= 2 and argv[1] == "--selftest":
        _selftest()
        return 0
    print("usage: a10_txlog_journal_docs.py --selftest", file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
