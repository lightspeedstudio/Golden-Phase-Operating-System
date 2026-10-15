#!/usr/bin/env python3
"""A18 classifier: secret-backed GPC1 checkpoint auth on one PZ7020 boot.

Does not open COM. Does not modify kernel output.
Host fixtures only. Secrets must not appear in UART.
Hardware-bound storage is unavailable; ephemeral RAM secret is this boot only.
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

EXIT_ACCEPT = 0
EXIT_REJECT = 1

_STATUS = re.compile(r"KEY_BACKEND:\s+(\S+)")


def _forbidden(text: str) -> str | None:
    if re.search(r"^result=PASS\s*$", text, flags=re.M):
        return "HOST_RESULT_LINE"
    if re.search(r"^===== .+ (PASS|FAIL)", text, flags=re.M):
        return "HOST_STEP_BANNER"
    return None


def _secret_leaked(text: str, secrets: list[str]) -> str | None:
    low = text.lower()
    for secret in secrets:
        s = secret.strip().lower()
        if len(s) < 16:
            continue
        if s in low:
            return "SECRET_IN_UART"
        try:
            raw = bytes.fromhex(s)
        except ValueError:
            raw = b""
        if raw and raw in text.encode("utf-8", "replace"):
            return "SECRET_IN_UART"
    return None


def classify(text: str, secret_a: str = "", secret_b: str = "") -> tuple[int, str, dict]:
    bad = _forbidden(text)
    if bad:
        return EXIT_REJECT, bad, {}
    ok, reason = is_raw_uart_evidence(text)
    if not ok:
        return EXIT_REJECT, reason, {}
    leak = _secret_leaked(text, [secret_a, secret_b])
    if leak:
        return EXIT_REJECT, leak, {}
    roots = boot_session_roots(text)
    if len(roots) != 1:
        return EXIT_REJECT, "BOOT_EPOCH", {"roots": roots}

    if "checkpoint-auth: provisioned EPHEMERAL_PROVISIONED_SECRET" not in text:
        return EXIT_REJECT, "NO_PROVISION", {"run_id": roots[0]}
    if text.count("checkpoint-auth: provisioned EPHEMERAL_PROVISIONED_SECRET") < 2:
        return EXIT_REJECT, "NEED_TWO_PROVISIONS", {"run_id": roots[0]}

    if "checkpoint-auth verify: OK" not in text:
        return EXIT_REJECT, "NO_VERIFY_OK", {"run_id": roots[0]}
    if "checkpoint-auth verify failed: BAD_SIGNATURE" not in text:
        return EXIT_REJECT, "NO_BAD_SIGNATURE", {"run_id": roots[0]}
    if "checkpoint-auth seal failed: AUTH_UNAVAILABLE" not in text:
        return EXIT_REJECT, "NO_INITIAL_AUTH_UNAVAILABLE_SEAL", {"run_id": roots[0]}

    if "HARDWARE_BOUND: NO" not in text:
        return EXIT_REJECT, "MISSING_HARDWARE_BOUND_NO", {"run_id": roots[0]}
    if "AUTH_UNAVAILABLE" not in text:
        return EXIT_REJECT, "MISSING_AUTH_UNAVAILABLE", {"run_id": roots[0]}
    if "EPHEMERAL_PROVISIONED_SECRET" not in text:
        return EXIT_REJECT, "MISSING_EPHEMERAL", {"run_id": roots[0]}
    if "not device-bound" not in text and "not persistent across reboot" not in text:
        return EXIT_REJECT, "MISSING_EPHEMERAL_HONESTY", {"run_id": roots[0]}
    if "HARDWARE_BOUND: YES" in text:
        return EXIT_REJECT, "FALSE_HARDWARE_BOUND", {"run_id": roots[0]}
    if re.search(r"KEY_BACKEND:\s+HARDWARE_BOUND_SECRET\s*$", text, flags=re.M) and "KEY_ERROR: UNAVAILABLE" not in text:
        return EXIT_REJECT, "HARDWARE_BACKEND_NOT_UNAVAILABLE", {"run_id": roots[0]}
    if "class: INTEGRITY_ONLY" not in text:
        return EXIT_REJECT, "MISSING_V1_INTEGRITY_ONLY", {"run_id": roots[0]}
    if "checkpoint-auth sealed" not in text:
        return EXIT_REJECT, "NO_SEAL", {"run_id": roots[0]}
    if re.search(r"checkpoint-auth sealed[\s\S]*signature:", text):
        return EXIT_REJECT, "HMAC_LABELED_SIGNATURE", {"run_id": roots[0]}

    return EXIT_ACCEPT, "CHECKPOINT_AUTH_A18", {"run_id": roots[0]}


def _selftest() -> None:
    secret_a = "aa" * 32
    secret_b = "bb" * 32
    uart = (
        "  [ SEALED] BootSessionRoot:     aabbccdd\n"
        "gpos> checkpoint-auth status\n"
        "KEY_BACKEND: AUTH_UNAVAILABLE\n"
        "HARDWARE_BOUND: NO\n"
        "gpos> checkpoint-auth hardware\n"
        "KEY_BACKEND: HARDWARE_BOUND_SECRET\n"
        "HARDWARE_BOUND: NO\n"
        "KEY_ERROR: UNAVAILABLE\n"
        "AUTH_UNAVAILABLE\n"
        "gpos> checkpoint-auth seal\n"
        "checkpoint-auth seal failed: AUTH_UNAVAILABLE\n"
        "gpos> checkpoint-auth verify\n"
        "checkpoint-auth verify failed: EMPTY\n"
        "gpos> checkpoint-auth provision\n"
        "checkpoint-auth secret (64 hex chars, no echo): \n"
        "checkpoint-auth: provisioned EPHEMERAL_PROVISIONED_SECRET (RAM this boot only)\n"
        "gpos> checkpoint-auth status\n"
        "KEY_BACKEND: EPHEMERAL_PROVISIONED_SECRET\n"
        "HARDWARE_BOUND: NO\n"
        "NOTE: not device-bound; not persistent across reboot\n"
        "gpos> checkpoint-auth seal\n"
        "checkpoint-auth sealed\n"
        "KEY_BACKEND: EPHEMERAL_PROVISIONED_SECRET\n"
        "HARDWARE_BOUND: NO\n"
        "hmac: abcd…\n"
        "gpos> checkpoint-auth verify\n"
        "checkpoint-auth verify: OK\n"
        "gpos> checkpoint-auth provision\n"
        "checkpoint-auth: provisioned EPHEMERAL_PROVISIONED_SECRET (RAM this boot only)\n"
        "gpos> checkpoint-auth verify\n"
        "checkpoint-auth verify failed: BAD_SIGNATURE\n"
        "gpos> checkpoint\n"
        "checkpoint sealed\n"
        "class: INTEGRITY_ONLY\n"
        "hmac: eeee…\n"
        "gpos>\n"
    )
    code, reason, info = classify(uart, secret_a, secret_b)
    assert code == EXIT_ACCEPT, (code, reason, info)
    leaked = uart.replace("hmac: abcd…", secret_a)
    code, reason, _info = classify(leaked, secret_a, secret_b)
    assert code == EXIT_REJECT and reason == "SECRET_IN_UART", (code, reason)


if __name__ == "__main__":
    _selftest()
    print("a18_checkpoint_auth_board selftest OK")
