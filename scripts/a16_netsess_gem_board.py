#!/usr/bin/env python3
"""A16 board classifier: live GEM RX take records on the netsess software ring.

Does not open COM. Does not modify kernel output.
Host fixtures only. Existing transcripts are not rewritten.
netsess demo/fault is not evidence. gem_rx_recorded must rise after MAC loopback.
Does not require a wire peer. Does not claim IOMMU or HwOwned.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

_SCRIPTS = Path(__file__).resolve().parent
_JOURNAL = _SCRIPTS.parent / "fpga" / "zynq" / "research" / "phasefs-persistent-journal-v1"
sys.path.insert(0, str(_JOURNAL))

from a01_raw_uart import (  # noqa: E402
    boot_session_roots,
    is_raw_uart_evidence,
)

LOOPBACK = re.compile(
    r"gem loopback: PASS rx_len=(\d+) \(MAC local only — not wire Ethernet\)"
)
GEM_RX = re.compile(r"gem_rx_recorded:\s+(\d+)")
GEM_LEN = re.compile(r"gem_rx_last_len:\s+(\d+)")

EXIT_ACCEPT = 0
EXIT_REJECT = 1


def _forbidden(text: str) -> str | None:
    if "journal i/o" in text:
        return "JOURNAL_IO"
    if re.search(r"^result=PASS\s*$", text, flags=re.M):
        return "HOST_RESULT_LINE"
    if re.search(r"^===== .+ (PASS|FAIL)", text, flags=re.M):
        return "HOST_STEP_BANNER"
    if re.search(r"DescriptorState::HwOwned|state=HwOwned", text):
        return "HWOWNED_IN_NETSESS"
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
        return EXIT_REJECT, "BOOT_EPOCH", {}
    lb = LOOPBACK.search(text)
    if not lb:
        return EXIT_REJECT, "NO_LOOPBACK_PASS", {}
    recorded = [int(m.group(1)) for m in GEM_RX.finditer(text)]
    if not recorded:
        return EXIT_REJECT, "NO_GEM_RX_RECORDED", {}
    if recorded[-1] < 1:
        return EXIT_REJECT, "GEM_RX_NOT_RECORDED", {"recorded": recorded}
    last_len = [int(m.group(1)) for m in GEM_LEN.finditer(text)]
    if not last_len or last_len[-1] < 1:
        return EXIT_REJECT, "NO_GEM_RX_LAST_LEN", {}
    return EXIT_ACCEPT, "NETSESS_A16", {
        "run_id": roots[0],
        "rx_len": int(lb.group(1)),
        "gem_rx_recorded": recorded[-1],
        "gem_rx_last_len": last_len[-1],
    }


def _selftest() -> None:
    boot = (
        "  [ SEALED] BootSessionRoot:     aabbccdd\n"
        "gpos> netsess\n"
        "netsess\n"
        "             sessions_live: 1\n"
        "             gem_rx_recorded: 0\n"
        "             gem_rx_last_len: 0\n"
        "gpos> net loopback\n"
        "gem loopback: PASS rx_len=64 (MAC local only — not wire Ethernet)\n"
        "gpos> netsess\n"
        "netsess\n"
        "             sessions_live: 1\n"
        "             gem_rx_recorded: 1\n"
        "             gem_rx_last_len: 64\n"
        "gpos>\n"
    )
    code, reason, info = classify(boot)
    assert code == EXIT_ACCEPT, (code, reason, info)
    missing = boot.replace(
        "gem loopback: PASS rx_len=64 (MAC local only — not wire Ethernet)\n",
        "gem loopback: FAIL timeout\n",
    )
    code, reason, _ = classify(missing)
    assert code == EXIT_REJECT and reason == "NO_LOOPBACK_PASS", (code, reason)
    zero = boot.replace("gem_rx_recorded: 1\n", "gem_rx_recorded: 0\n")
    code, reason, _ = classify(zero)
    assert code == EXIT_REJECT and reason == "GEM_RX_NOT_RECORDED", (code, reason)


if __name__ == "__main__":
    _selftest()
    if len(sys.argv) == 1:
        print("selftest ok")
        raise SystemExit(0)
    text = Path(sys.argv[1]).read_text(encoding="utf-8", errors="replace")
    code, reason, info = classify(text)
    print(reason, info)
    raise SystemExit(code)
