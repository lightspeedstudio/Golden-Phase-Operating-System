#!/usr/bin/env python3
"""A03: MMU matrix UART must be one BootSessionRoot, 10 rows in order, once.

Does not open COM. Does not modify kernel output. Host fixtures only.
Existing COM12 transcripts are not rewritten.

Old A03-era acceptance (capture_mmuiso_com12.ps1 before this file):
  $pass = count of lines matching
    ^(legal-read|kernel-read|kernel-write|peer-read|peer-write|
      mmio-write|execute-data|stack-overflow): PASS$
  exit 0 iff Send-Cmd saw gpos> AND $pass > 0
  No run ID. No order. No uniqueness. code-read/code-write not required.
  gpos> is transport ($ok), incorrectly mixed into PASS via A02's pass>0 only.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

EXPECTED_ROWS: tuple[str, ...] = (
    "legal-read",
    "code-read",
    "kernel-read",
    "kernel-write",
    "peer-read",
    "peer-write",
    "mmio-write",
    "code-write",
    "execute-data",
    "stack-overflow",
)
EXPECTED_PASS_TOTAL = 10
assert len(EXPECTED_ROWS) == EXPECTED_PASS_TOTAL

OLD_COUNT_NAMES = (
    "legal-read",
    "kernel-read",
    "kernel-write",
    "peer-read",
    "peer-write",
    "mmio-write",
    "execute-data",
    "stack-overflow",
)

ROW_RE = re.compile(
    r"^(" + "|".join(re.escape(n) for n in EXPECTED_ROWS) + r"): PASS\s*$"
)
ROOT_RE = re.compile(r"BootSessionRoot:\s+([0-9a-fA-F]+)")
DONE_LINE = "mmuiso matrix done"

# Distinct from A02 timeout (4) and A02 zero-pass (5).
EXIT_ACCEPT = 0
EXIT_ZERO_PASS = 5
EXIT_REJECT = 6


def old_a03_accept(text: str, commands_got_prompt: bool = True) -> bool:
    """Pre-A03 rule: any old-name PASS count > 0 plus prompt. No order/run ID."""
    if not commands_got_prompt:
        return False
    pat = r"^(?:" + "|".join(re.escape(n) for n in OLD_COUNT_NAMES) + r"): PASS\s*$"
    n = len(re.findall(pat, text, flags=re.M))
    return n > 0


def classify_mmuiso_matrix(text: str) -> tuple[int, str, str]:
    """Return (exit_code, reason, run_id). run_id is BootSessionRoot or empty."""
    if not text:
        return EXIT_ZERO_PASS, "EMPTY", ""

    roots: list[tuple[int, str]] = []
    rows: list[tuple[int, str, str]] = []
    dones: list[int] = []
    current_root = ""

    for i, line in enumerate(text.splitlines()):
        rm = ROOT_RE.search(line)
        if rm:
            current_root = rm.group(1).lower()
            roots.append((i, current_root))
            continue
        stripped = line.rstrip("\r")
        m = ROW_RE.match(stripped)
        if m:
            rows.append((i, m.group(1), current_root))
            continue
        if stripped == DONE_LINE:
            dones.append(i)

    distinct_roots = list(dict.fromkeys(r for _, r in roots))
    if len(distinct_roots) == 0:
        return EXIT_REJECT, "NO_RUN_ID", ""
    if len(distinct_roots) > 1:
        return EXIT_REJECT, "MIXED_BOOT_EPOCH", distinct_roots[0]
    run_id = distinct_roots[0]
    first_root_idx = roots[0][0]

    if any(idx < first_root_idx for idx, _, _ in rows):
        return EXIT_REJECT, "STALE_PRIOR_BOOT", run_id
    if any(rid != run_id for _, _, rid in rows):
        return EXIT_REJECT, "MIXED_RUN_ID", run_id

    rows_after_root = [(idx, name) for idx, name, _ in rows if idx > first_root_idx]
    if not rows_after_root:
        return EXIT_ZERO_PASS, "ZERO_PASS", run_id
    if len(dones) != 1:
        return EXIT_REJECT, "DUPLICATE_OR_MISSING_DONE", run_id

    done_idx = dones[0]
    window = [(idx, name) for idx, name, _ in rows if first_root_idx < idx < done_idx]
    after = [(idx, name) for idx, name, _ in rows if idx > done_idx]
    if after:
        return EXIT_REJECT, "ROWS_AFTER_DONE", run_id
    if not window:
        return EXIT_ZERO_PASS, "ZERO_PASS", run_id

    names = [n for _, n in window]
    if len(names) != EXPECTED_PASS_TOTAL:
        if len(set(names)) < len(names):
            return EXIT_REJECT, "DUPLICATE_ROW", run_id
        missing = [n for n in EXPECTED_ROWS if n not in names]
        if missing:
            return EXIT_REJECT, "MISSING_ROW", run_id
        return EXIT_REJECT, "WRONG_PASS_TOTAL", run_id
    if names != list(EXPECTED_ROWS):
        if len(set(names)) < len(names):
            return EXIT_REJECT, "DUPLICATE_ROW", run_id
        if set(names) == set(EXPECTED_ROWS):
            return EXIT_REJECT, "REORDERED", run_id
        return EXIT_REJECT, "MISSING_ROW", run_id
    return EXIT_ACCEPT, "ORDERED_SINGLE_RUN", run_id


def _ordered_block(run_id: str) -> str:
    lines = [f"[ SEALED] BootSessionRoot:     {run_id}"]
    lines.extend(f"{n}: PASS" for n in EXPECTED_ROWS)
    lines.append(DONE_LINE)
    return "\n".join(lines) + "\n"


def _selftest() -> None:
    good = "gpos> mmuiso selftest\n" + _ordered_block("fa82a350") + "gpos>\n"
    code, reason, rid = classify_mmuiso_matrix(good)
    assert code == EXIT_ACCEPT and reason == "ORDERED_SINGLE_RUN" and rid == "fa82a350", (
        code,
        reason,
        rid,
    )
    assert old_a03_accept(good) is True

    # Different run IDs / boot epochs. Old count still > 0.
    mixed = (
        _ordered_block("aaaaaaaa")
        + "gpos> mmuiso selftest\n"
        + _ordered_block("bbbbbbbb")
    )
    assert old_a03_accept(mixed) is True
    code, reason, _ = classify_mmuiso_matrix(mixed)
    assert code == EXIT_REJECT and reason == "MIXED_BOOT_EPOCH", (code, reason)

    # Correct names, wrong order.
    reordered = "[ SEALED] BootSessionRoot:     fa82a350\n"
    reordered += "\n".join(f"{n}: PASS" for n in reversed(EXPECTED_ROWS))
    reordered += f"\n{DONE_LINE}\n"
    assert old_a03_accept(reordered) is True
    code, reason, _ = classify_mmuiso_matrix(reordered)
    assert code == EXIT_REJECT and reason == "REORDERED", (code, reason)

    # Stale markers from an earlier boot, then a new root with no rows.
    stale = _ordered_block("11111111") + "[ SEALED] BootSessionRoot:     22222222\n"
    assert old_a03_accept(stale) is True
    code, reason, _ = classify_mmuiso_matrix(stale)
    assert code == EXIT_REJECT and reason == "MIXED_BOOT_EPOCH", (code, reason)

    stale2 = (
        "legal-read: PASS\nkernel-read: PASS\n"
        + _ordered_block("fa82a350")
    )
    assert old_a03_accept(stale2) is True
    code, reason, _ = classify_mmuiso_matrix(stale2)
    assert code == EXIT_REJECT and reason == "STALE_PRIOR_BOOT", (code, reason)

    # Missing markers (no stack-overflow). Old 8-name regex still matches 7+.
    missing = "[ SEALED] BootSessionRoot:     fa82a350\n"
    missing += "\n".join(f"{n}: PASS" for n in EXPECTED_ROWS[:-1])
    missing += f"\n{DONE_LINE}\n"
    assert old_a03_accept(missing) is True
    code, reason, _ = classify_mmuiso_matrix(missing)
    assert code == EXIT_REJECT and reason == "MISSING_ROW", (code, reason)

    # pass=0: prompt and root only. Old A03 (count) is false; must stay nonzero.
    zero = "[ SEALED] BootSessionRoot:     fa82a350\ngpos>\n"
    assert old_a03_accept(zero) is False
    code, reason, _ = classify_mmuiso_matrix(zero)
    assert code == EXIT_ZERO_PASS and reason == "ZERO_PASS", (code, reason)

    # Duplicate row (selftest + legal-read). Old count still > 0.
    dup = _ordered_block("fa82a350") + "legal-read: PASS\n"
    assert old_a03_accept(dup) is True
    code, reason, _ = classify_mmuiso_matrix(dup)
    assert code == EXIT_REJECT and reason in (
        "ROWS_AFTER_DONE",
        "DUPLICATE_ROW",
    ), (code, reason)

    print("a03_mmuiso_matrix selftest: ok")


def main(argv: list[str]) -> int:
    if len(argv) >= 2 and argv[1] == "--selftest":
        _selftest()
        return 0
    if len(argv) < 2:
        print("usage: a03_mmuiso_matrix.py <uart.txt>|--selftest", file=sys.stderr)
        return 2
    path = Path(argv[1])
    text = path.read_text(encoding="utf-8", errors="replace")
    code, reason, run_id = classify_mmuiso_matrix(text)
    print(f"file={path.name}")
    print(f"A03_ACCEPT={'YES' if code == EXIT_ACCEPT else 'NO'}")
    print(f"reason={reason}")
    print(f"run_id={run_id}")
    print(f"expected_pass_total={EXPECTED_PASS_TOTAL}")
    return code


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
