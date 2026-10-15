#!/usr/bin/env python3
"""A07: journal_sd.rs must not claim research-only / BOOT_FSJOURNAL_RESEARCH.BIN.

Does not format the SD journal. Does not talk to the board.
"""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
JOURNAL_SD = ROOT / "src" / "board" / "zynq7000" / "journal_sd.rs"
CARGO = ROOT / "Cargo.toml"

OLD_MODULE = "SD partition-1 window for the PhaseFS journal. Research-only."
OLD_BOOT = "Distinct research BOOT name: `BOOT_FSJOURNAL_RESEARCH.BIN`."


def claims_research_only_boot(src: str) -> bool:
    return (
        "The PhaseFS journal. Research-only." in src
        or "Distinct research BOOT name:" in src
        or OLD_MODULE in src
        or OLD_BOOT in src
    )


def utility_includes_phasefs_journal(cargo: str) -> bool:
    start = cargo.find("utility = [")
    if start < 0:
        return False
    end = cargo.find("]", start)
    if end < 0:
        return False
    return '"phasefs-journal"' in cargo[start : end + 1]


def _selftest() -> None:
    assert claims_research_only_boot(OLD_MODULE) is True
    assert claims_research_only_boot(OLD_BOOT) is True
    honest = (
        "SD partition-1 window for the PhaseFS journal "
        "(`feature = \"phasefs-journal\"`).\n"
        "Gold Utility includes this feature."
    )
    assert claims_research_only_boot(honest) is False

    src = JOURNAL_SD.read_text(encoding="utf-8")
    assert not claims_research_only_boot(src), (
        "A07: journal_sd.rs still claims research-only / "
        "BOOT_FSJOURNAL_RESEARCH.BIN"
    )
    assert "Does not write the MBR." in src
    assert "Never treats partition 0 as the journal." in src

    cargo = CARGO.read_text(encoding="utf-8")
    assert utility_includes_phasefs_journal(cargo), (
        "A07: Cargo.toml utility feature must still include phasefs-journal"
    )
    print("a07_journal_sd_docs selftest: ok")


def main(argv: list[str]) -> int:
    if len(argv) >= 2 and argv[1] == "--selftest":
        _selftest()
        return 0
    print("usage: a07_journal_sd_docs.py --selftest", file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
