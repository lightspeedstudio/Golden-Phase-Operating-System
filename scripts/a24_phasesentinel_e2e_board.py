#!/usr/bin/env python3
"""A24 classifier: contain over-cap, then signed hello still launches.

Does not open COM. Does not modify kernel output.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

_SCRIPTS = Path(__file__).resolve().parent
_JOURNAL = _SCRIPTS.parent / "fpga" / "zynq" / "research" / "phasefs-persistent-journal-v1"
sys.path.insert(0, str(_SCRIPTS))
sys.path.insert(0, str(_JOURNAL))

from a01_raw_uart import boot_session_roots, is_raw_uart_evidence  # noqa: E402

EXIT_ACCEPT = 0
EXIT_REJECT = 1

_ALLOW = "sentinel=Allow"
_ESC = "CapabilityEscalation"
_REVOKED = "Revoked"
_BOUND = "user_map: bound"
_REMOVED = "user_map: removed"


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
    if _BOUND not in text:
        return EXIT_REJECT, "NO_USER_MAP_BOUND", info
    if _REMOVED not in text:
        return EXIT_REJECT, "NO_USER_MAP_REMOVED", info
    esc = text.find(_ESC)
    if esc < 0:
        return EXIT_REJECT, "NO_CONTAINMENT", info
    after = text[esc:]
    if _ALLOW not in after:
        return EXIT_REJECT, "NO_VALID_LAUNCH_AFTER_CONTAIN", info
    allow_at = after.find(_ALLOW)
    if _REVOKED not in after[allow_at:]:
        return EXIT_REJECT, "NO_REVOKE_AFTER_SURVIVING_LAUNCH", info
    for needle, tag in (
        ("MissingEntry", "NO_BAD"),
        ("UnknownCert", "NO_UNK"),
        ("RetiredSigner", "NO_OLD"),
        ("Unsigned", "NO_NOSIG"),
    ):
        if needle not in text:
            return EXIT_REJECT, tag, info
    if "gpos>" not in text.splitlines()[-3:]:
        # last lines may be checkpoint output then prompt
        if not text.rstrip().endswith("gpos>") and "gpos> " not in text[-20:]:
            return EXIT_REJECT, "SHELL_DOWN", info
    return EXIT_ACCEPT, "PHASESENTINEL_E2E", info


def _selftest() -> None:
    uart = (
        "  [ SEALED] BootSessionRoot:     aabbccdd\n"
        "installed app=hello-a7f3c91e root=0210faec\n"
        "user_map: bound scope=User\n"
        "sentinel=Allow\n"
        "run failed: CapabilityEscalation\n"
        "user_map: removed granted=0x0\n"
        "sentinel=Allow\n"
        "user_map: removed granted=0x0\n"
        "run failed: Revoked\n"
        "install failed: MissingEntry\n"
        "install failed: UnknownCert\n"
        "install failed: RetiredSigner\n"
        "install failed: Unsigned\n"
        "gpos>\n"
    )
    code, reason, info = classify(uart)
    assert code == EXIT_ACCEPT, (code, reason, info)
    code, reason, _ = classify(uart.replace(_ESC, "ok"))
    assert code == EXIT_REJECT and reason == "NO_CONTAINMENT", (code, reason)
    no_second = uart.replace(_ALLOW, "deny", 1)
    # first Allow removed; second remains AFTER esc — still PASS
    code, reason, _ = classify(no_second)
    assert code == EXIT_ACCEPT, (code, reason)
    both_allow_gone = uart.replace(_ALLOW, "x")
    code, reason, _ = classify(both_allow_gone)
    assert code == EXIT_REJECT and reason == "NO_VALID_LAUNCH_AFTER_CONTAIN", (code, reason)
    mixed = "result=PASS\n" + uart
    code, reason, _ = classify(mixed)
    assert code == EXIT_REJECT and reason == "HOST_RESULT_LINE", (code, reason)


if __name__ == "__main__":
    _selftest()
    print("a24_phasesentinel_e2e_board selftest OK")
