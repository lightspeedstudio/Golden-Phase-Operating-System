#!/usr/bin/env python3
"""A06: pmm.rs must not claim MMU off / phys==virt.

Does not allocate frames. Does not talk to the board.
"""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PMM = ROOT / "src" / "kernel" / "pmm.rs"

OLD_MODULE = (
    "No MMU/page-table code. The board runs with the MMU off (phys == virt), so\n"
    '"physical address" and "address" are the same number here.'
)
OLD_ALLOC = "physical (== virtual; MMU off) start address."


def claims_mmu_off(src: str) -> bool:
    return (
        "The board runs with the MMU off" in src
        or "phys == virt" in src
        or OLD_ALLOC in src
    )


def _selftest() -> None:
    assert claims_mmu_off(OLD_MODULE) is True
    assert claims_mmu_off(OLD_ALLOC) is True
    assert claims_mmu_off("No MMU/page-table code. alloc returns a physical frame address.") is False

    src = PMM.read_text(encoding="utf-8")
    assert not claims_mmu_off(src), "A06: pmm.rs still claims MMU off / phys==virt"
    assert "No MMU/page-table code." in src
    print("a06_pmm_docs selftest: ok")


def main(argv: list[str]) -> int:
    if len(argv) >= 2 and argv[1] == "--selftest":
        _selftest()
        return 0
    print("usage: a06_pmm_docs.py --selftest", file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
