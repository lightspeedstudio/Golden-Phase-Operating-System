#!/usr/bin/env python3
"""A04: rodata listen harness — raw UART file vs listen_done count PASS_BOOT.

Ledger: capture_mmuiso_rodata_fix.py has no append-only raw UART file;
emit(..., listen_done) mixes counts + full text; PASS_BOOT from
n_gpos and n_g1p_done and n_aligned_yes.

Does not open COM. Does not change kernel strings. Does not rewrite UART logs.
"""

from __future__ import annotations

import hashlib
import json
import re
import sys
from pathlib import Path

HOST_PARSER_EVENT = "HOST_PARSER_EVENT"
ROOT_RE = re.compile(r"BootSessionRoot:\s+([0-9a-fA-F]+)")
PROMPT_LINE = re.compile(r"^gpos>\s*$")

EXIT_ACCEPT = 0
EXIT_REJECT = 6


def old_pass_boot(text: str) -> bool:
    """Exact pre-A04 verdict: substring counts, any order, splash gpos> counts."""
    n_gpos = text.count("gpos>")
    n_g1p_done = text.count("G1P_DONE")
    n_aligned_yes = text.count("aligned=YES")
    return bool(n_gpos and n_g1p_done and n_aligned_yes)


def listen_done_is_contaminated(record: dict) -> bool:
    """True if listen_done carries counts plus full UART (old emit payload)."""
    if record.get("message") != "listen_done":
        return False
    data = record.get("data") or {}
    return "full" in data and "n_gpos" in data and "n_g1p_done" in data and "n_aligned_yes" in data


def classify_rodata_boot_uart(text: str) -> tuple[int, str, str]:
    """PASS_BOOT only from one BootSessionRoot and ordered board lines, not counts."""
    if not text:
        return EXIT_REJECT, "EMPTY", ""
    try:
        rec = json.loads(text.splitlines()[0])
    except json.JSONDecodeError:
        rec = None
    if isinstance(rec, dict) and listen_done_is_contaminated(rec):
        return EXIT_REJECT, "LISTEN_DONE_MIXED", ""
    if isinstance(rec, dict) and rec.get("message") == "listen_done":
        return EXIT_REJECT, "HOST_LISTEN_DONE", ""

    roots: list[tuple[int, str]] = []
    aligned: list[int] = []
    g1p: list[int] = []
    prompts: list[int] = []
    for i, line in enumerate(text.splitlines()):
        rm = ROOT_RE.search(line)
        if rm:
            roots.append((i, rm.group(1).lower()))
        if "aligned=YES" in line:
            aligned.append(i)
        if line.rstrip("\r") == "G1P_DONE":
            g1p.append(i)
        if PROMPT_LINE.match(line.rstrip("\r")):
            prompts.append(i)

    distinct = list(dict.fromkeys(r for _, r in roots))
    if not distinct:
        return EXIT_REJECT, "NO_RUN_ID", ""
    if len(distinct) > 1:
        return EXIT_REJECT, "MIXED_BOOT_EPOCH", distinct[0]
    run_id = distinct[0]
    root_i = roots[0][0]
    a = next((i for i in aligned if i > root_i), None)
    g = next((i for i in g1p if i > root_i), None)
    p = next((i for i in prompts if i > root_i), None)
    if a is None or g is None or p is None:
        return EXIT_REJECT, "MISSING_ORDERED_MARKERS", run_id
    if not (root_i < a < g < p):
        return EXIT_REJECT, "REORDERED", run_id
    return EXIT_ACCEPT, "ORDERED_BOOT", run_id


def append_uart(path: Path, chunk: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("ab") as f:
        f.write(chunk)


def write_host_event(path: Path, message: str, data: dict) -> None:
    rec = {"type": HOST_PARSER_EVENT, "message": message, "data": data}
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as f:
        f.write(json.dumps(rec, separators=(",", ":")) + "\n")


def _selftest() -> None:
    splash = "T H E   M O M E N T   I S   N O W   -- type help at gpos>          | GPOS\n"
    unordered = splash + "G1P_DONE\naligned=YES\n"
    assert old_pass_boot(unordered) is True
    code, reason, _ = classify_rodata_boot_uart(unordered)
    assert code == EXIT_REJECT and reason == "NO_RUN_ID", (code, reason)

    mixed_listen = json.dumps(
        {
            "message": "listen_done",
            "data": {
                "n_gpos": 1,
                "n_g1p_done": 1,
                "n_aligned_yes": 1,
                "full": unordered,
            },
        }
    )
    assert listen_done_is_contaminated(json.loads(mixed_listen)) is True
    code, reason, _ = classify_rodata_boot_uart(mixed_listen)
    assert code == EXIT_REJECT and reason == "LISTEN_DONE_MIXED", (code, reason)

    good = (
        "[ SEALED] BootSessionRoot:     fa82a350\n"
        "MMU: ro=002e5000..002e6000 data=002e6000 aligned=YES\n"
        "G1P_DONE\n"
        "gpos>\n"
    )
    assert old_pass_boot(good) is True
    code, reason, rid = classify_rodata_boot_uart(good)
    assert code == EXIT_ACCEPT and reason == "ORDERED_BOOT" and rid == "fa82a350", (
        code,
        reason,
        rid,
    )

    reordered = (
        "[ SEALED] BootSessionRoot:     fa82a350\n"
        "G1P_DONE\n"
        "MMU: ro=0 aligned=YES\n"
        "gpos>\n"
    )
    assert old_pass_boot(reordered) is True
    code, reason, _ = classify_rodata_boot_uart(reordered)
    assert code == EXIT_REJECT and reason == "REORDERED", (code, reason)

    splash_only_prompt = (
        "[ SEALED] BootSessionRoot:     fa82a350\n"
        "MMU: aligned=YES\n"
        "G1P_DONE\n"
        + splash
    )
    assert old_pass_boot(splash_only_prompt) is True
    code, reason, _ = classify_rodata_boot_uart(splash_only_prompt)
    assert code == EXIT_REJECT and reason == "MISSING_ORDERED_MARKERS", (code, reason)

    print("a04_rodata_uart selftest: ok")


def main(argv: list[str]) -> int:
    if len(argv) >= 2 and argv[1] == "--selftest":
        _selftest()
        return 0
    if len(argv) < 2:
        print("usage: a04_rodata_uart.py <uart-or-json>|--selftest", file=sys.stderr)
        return 2
    path = Path(argv[1])
    raw = path.read_bytes()
    text = raw.decode("utf-8", "replace")
    code, reason, run_id = classify_rodata_boot_uart(text)
    print(f"file={path.name}")
    print(f"sha256={hashlib.sha256(raw).hexdigest()}")
    print(f"A04_ACCEPT={'YES' if code == EXIT_ACCEPT else 'NO'}")
    print(f"reason={reason}")
    print(f"run_id={run_id}")
    return code


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
