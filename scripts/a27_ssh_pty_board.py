#!/usr/bin/env python3
"""A27 classifier: interactive SSH PTY on UART audit + serve summary.

Does not open COM. Client transcript is host evidence.
UART must show pty open, a command, session close, serve shells>=1, then a
post-serve UART prompt.
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

_PTY = "ssh audit: pty open"
_CMD = "ssh audit: pty cmd"
_CLOSE = "ssh audit: session close"
_SERVE = re.compile(
    r"net ssh: served 192\.168\.5\.50/24 :2222 for \d+s "
    r"\(conns=(\d+) authed=(\d+) execs=(\d+) shells=(\d+)\)"
)
_KEY = "net ssh key: authorized pubkey set (not committed)"


def _forbidden(text: str) -> str | None:
    if re.search(r"^result=PASS\s*$", text, flags=re.M):
        return "HOST_RESULT_LINE"
    if re.search(r"^===== .+ (PASS|FAIL)", text, flags=re.M):
        return "HOST_STEP_BANNER"
    return None


def classify(text: str, client: str = "") -> tuple[int, str, dict]:
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
    if _KEY not in text:
        return EXIT_REJECT, "NO_RUNTIME_KEY", info
    if _PTY not in text:
        return EXIT_REJECT, "NO_PTY_OPEN", info
    if _CMD not in text:
        return EXIT_REJECT, "NO_PTY_CMD", info
    if _CLOSE not in text:
        return EXIT_REJECT, "NO_SESSION_CLOSE", info
    m = _SERVE.search(text)
    if not m:
        return EXIT_REJECT, "NO_SERVE_SUMMARY", info
    conns, authed, _execs, shells = (int(x) for x in m.groups())
    info.update({"conns": conns, "authed": authed, "shells": shells})
    if authed < 1 or shells < 1 or conns < 1:
        return EXIT_REJECT, "NO_PTY_SESSION", info
    if "gpos>" not in text[-80:]:
        return EXIT_REJECT, "UART_DOWN_AFTER_SSH", info
    if client:
        if "gpos>" not in client:
            return EXIT_REJECT, "CLIENT_NO_PROMPT", info
        if "fsj format refused: type CONFIRM" not in client:
            return EXIT_REJECT, "CLIENT_DENIED_MISSING", info
        if "AUTH_FAIL" not in client and "Permission denied" not in client:
            return EXIT_REJECT, "CLIENT_NEG_AUTH_MISSING", info
    return EXIT_ACCEPT, "SSH_PTY", info


def _selftest() -> None:
    uart = (
        "  [ SEALED] BootSessionRoot:     aabbccdd\n"
        "net ssh key: authorized pubkey set (not committed)\n"
        "net ssh: starting SSH server on 192.168.5.50:2222 for 90s ...\n"
        "ssh audit: pty open\n"
        "ssh audit: pty cmd\n"
        "ssh audit: session close\n"
        "net ssh: served 192.168.5.50/24 :2222 for 90s (conns=2 authed=1 execs=0 shells=1)\n"
        "gpos>\n"
    )
    client = "gpos> checkpoint\nfsj format refused: type CONFIRM\nPermission denied (publickey).\n"
    code, reason, info = classify(uart, client)
    assert code == EXIT_ACCEPT, (code, reason, info)
    code, reason, _ = classify(uart.replace(_PTY, "x"), client)
    assert code == EXIT_REJECT and reason == "NO_PTY_OPEN", (code, reason)
    mixed = "result=PASS\n" + uart
    code, reason, _ = classify(mixed, client)
    assert code == EXIT_REJECT and reason == "HOST_RESULT_LINE", (code, reason)


if __name__ == "__main__":
    _selftest()
    print("a27_ssh_pty_board selftest OK")
