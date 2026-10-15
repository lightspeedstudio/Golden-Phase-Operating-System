#!/usr/bin/env python3
"""A12: journal recover must rebuild extmap extents from PhaseFs.

Does not talk to the board.
"""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
KERNEL = ROOT / "src" / "kernel" / "kernel.rs"
EXTMAP = ROOT / "src" / "kernel" / "extmap.rs"


def claims_recover_skips_extmap(kernel_src: str) -> bool:
    start = kernel_src.find("pub fn journal_post_recover")
    if start < 0:
        return True
    body = kernel_src[start : start + 1800]
    return "write_inode" not in body or "InodeKind::File" not in body


def _selftest() -> None:
    old = "pub fn journal_post_recover(&mut self) {\n        self.phase_index = PhaseIndexController::new();\n"
    assert claims_recover_skips_extmap(old) is True
    kernel_src = KERNEL.read_text(encoding="utf-8")
    assert not claims_recover_skips_extmap(kernel_src), (
        "A12: journal_post_recover still skips extmap rebuild"
    )
    extmap_src = EXTMAP.read_text(encoding="utf-8")
    assert "journal_post_recover" in extmap_src
    assert "X_before/X_fault" not in extmap_src
    print("a12_extmap_recover selftest: ok")


def main(argv: list[str]) -> int:
    if len(argv) >= 2 and argv[1] == "--selftest":
        _selftest()
        return 0
    print("usage: a12_extmap_recover.py --selftest", file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
