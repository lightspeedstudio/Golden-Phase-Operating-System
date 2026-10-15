#!/usr/bin/env python3
"""A21 classifier: one Utility+journal boot, one UART, MMU 10/10 + fsj check.

Does not open COM. Does not modify kernel output.
FCFA3F38 remains unpacked-without-matrix; this classifies a later combined image.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

_SCRIPTS = Path(__file__).resolve().parent
_JOURNAL = _SCRIPTS.parent / "fpga" / "zynq" / "research" / "phasefs-persistent-journal-v1"
sys.path.insert(0, str(_SCRIPTS))
sys.path.insert(0, str(_JOURNAL))

from a01_raw_uart import (  # noqa: E402
    board_check_durable_in_raw,
    boot_session_roots,
    is_raw_uart_evidence,
)
from a03_mmuiso_matrix import EXIT_ACCEPT as MMU_ACCEPT  # noqa: E402
from a03_mmuiso_matrix import classify_mmuiso_matrix  # noqa: E402

EXIT_ACCEPT = 0
EXIT_REJECT = 1


def _forbidden(text: str) -> str | None:
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
        return EXIT_REJECT, "BOOT_EPOCH", {"roots": roots}
    info = {"run_id": roots[0]}
    if "MMU: CPU0 isolation" not in text:
        return EXIT_REJECT, "NO_MMU_ISOLATION", info
    if "G1P_DONE" not in text:
        return EXIT_REJECT, "NO_G1P_AXI", info
    if "start_lba=15706112" not in text or "sectors=16384" not in text:
        return EXIT_REJECT, "NO_JOURNAL_P1", info
    mcode, mreason, mrun = classify_mmuiso_matrix(text)
    info["mmu"] = mreason
    if mcode != MMU_ACCEPT:
        return EXIT_REJECT, f"MMU_{mreason}", info
    if mrun != roots[0]:
        return EXIT_REJECT, "MMU_RUN_ID_MISMATCH", info
    jok, jreason = board_check_durable_in_raw(text)
    info["journal"] = jreason
    if not jok:
        return EXIT_REJECT, f"JOURNAL_{jreason}", info
    if "SD_PARTITION_1" not in text:
        return EXIT_REJECT, "NO_FSJ_STATUS_SD", info
    return EXIT_ACCEPT, "COMBINED_MATRIX_A21", info


def _selftest() -> None:
    from a03_mmuiso_matrix import EXPECTED_ROWS

    rows = "\n".join(f"{n}: PASS" for n in EXPECTED_ROWS)
    uart = (
        "MMU: CPU0 isolation (Client D0)\n"
        "fsj: partition 1 start_lba=15706112 sectors=16384 p1_end=15722496\n"
        "  [ SEALED] BootSessionRoot:     aabbccdd\n"
        "G1P_DONE\n"
        "gpos> mmuiso matrix\n"
        f"{rows}\n"
        "mmuiso matrix done\n"
        "gpos> fsj status\n"
        "backend: SD_PARTITION_1\n"
        "gpos> fsj check\n"
        "fsj check: PASS\n"
        "DURABLE: YES\n"
        "gpos>\n"
    )
    code, reason, info = classify(uart)
    assert code == EXIT_ACCEPT, (code, reason, info)
    mixed = "result=PASS\n" + uart
    code, reason, _info = classify(mixed)
    assert code == EXIT_REJECT and reason == "HOST_RESULT_LINE", (code, reason)


if __name__ == "__main__":
    _selftest()
    print("a21_combined_matrix_board selftest OK")
