#!/usr/bin/env python3
"""A11: GPA install must persist through persist_write_file, not txlog.

Does not talk to the board.
"""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
INSTALL = ROOT / "src" / "kernel" / "phasestack" / "install.rs"

OLD_TXOP = "TxOp::WriteFile"
OLD_RUN = ".txlog"


def claims_install_uses_txlog_not_persist(src: str) -> bool:
    return OLD_TXOP in src or "txlog\n            .run_transaction" in src or (
        ".txlog" in src and "persist_write_file" not in src
    )


def _selftest() -> None:
    assert claims_install_uses_txlog_not_persist(OLD_TXOP) is True
    honest = (
        "write_store(kernel, path, data) calls persist_write_file.\n"
        "WAL success skips txlog."
    )
    assert claims_install_uses_txlog_not_persist(honest) is False

    src = INSTALL.read_text(encoding="utf-8")
    assert not claims_install_uses_txlog_not_persist(src), (
        "A11: install.rs still writes GPA store through txlog"
    )
    assert "persist_write_file" in src
    assert "persist_mkdir" in src
    assert OLD_TXOP not in src
    print("a11_install_journal selftest: ok")


def main(argv: list[str]) -> int:
    if len(argv) >= 2 and argv[1] == "--selftest":
        _selftest()
        return 0
    print("usage: a11_install_journal.py --selftest", file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
