#!/usr/bin/env python3
"""A09: netsess.rs must not claim this module keeps GEM descriptor rings.

Does not send packets. Does not talk to the board.
"""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
NETSESS = ROOT / "src" / "kernel" / "netsess.rs"

OLD_GOAL = "real GEM descriptor rings"
OLD_SM = "Free -> HwOwned -> KernelReady"


def claims_owns_gem_rings(src: str) -> bool:
    return OLD_GOAL in src or OLD_SM in src


def descriptor_state_enum(src: str) -> str:
    start = src.find("pub enum DescriptorState")
    if start < 0:
        return ""
    end = src.find("}", start)
    if end < 0:
        return ""
    return src[start : end + 1]


def _selftest() -> None:
    assert claims_owns_gem_rings(OLD_GOAL) is True
    assert claims_owns_gem_rings(OLD_SM) is True
    honest = (
        "Additive software session table and descriptor ring. This module does\n"
        "**not** own GEM DMA descriptors and is not wired to gem hardware rings."
    )
    assert claims_owns_gem_rings(honest) is False

    src = NETSESS.read_text(encoding="utf-8")
    assert not claims_owns_gem_rings(src), (
        "A09: netsess.rs still implies this module keeps GEM rings / HwOwned"
    )
    assert "**not** own GEM DMA descriptors" in src
    assert "software session table" in src
    enum = descriptor_state_enum(src)
    assert "KernelReady" in enum
    assert "HwOwned" not in enum
    print("a09_netsess_docs selftest: ok")


def main(argv: list[str]) -> int:
    if len(argv) >= 2 and argv[1] == "--selftest":
        _selftest()
        return 0
    print("usage: a09_netsess_docs.py --selftest", file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
