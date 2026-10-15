#!/usr/bin/env python3
"""A05: prior MMU 10/10 and journal baseline claims that used A01–A04 harnesses.

Does not open COM. Does not rewrite UART files. Does not mint a board PASS.
Host fixtures and frozen historical hashes only.
"""

from __future__ import annotations

import hashlib
import re
import sys
from pathlib import Path

_ROOT = Path(__file__).resolve().parents[1]
_JOURNAL = _ROOT / "fpga" / "zynq" / "research" / "phasefs-persistent-journal-v1"
sys.path.insert(0, str(_JOURNAL))

from a01_raw_uart import is_raw_uart_evidence  # noqa: E402
from a03_mmuiso_matrix import (  # noqa: E402
    EXIT_ACCEPT,
    classify_mmuiso_matrix,
    old_a03_accept,
)

MMU_EVIDENCE = (
    _ROOT
    / "fpga"
    / "zynq"
    / "evidence"
    / "kairos_zynq_boot"
    / "BOOT_MMU_ISOLATION_GOAL5_20260816"
    / "com12_matrix.txt"
)
JOURNAL_EVIDENCE = _JOURNAL / "com12_fsjournal_20260814_222341.txt"
AUDIT_DOC = _ROOT / "docs" / "research" / "ARMV7_MMU_ISOLATION_AUDIT.md"
STATUS_DOC = _JOURNAL / "STATUS.md"

# Frozen at A05 open. Byte-for-byte; do not rewrite.
MMU_SHA256 = "23cea7b111eed5ebe678f8b855345b7029f5b7073713d7f84ede8b3f12465f45"
MMU_BYTES = 25088
JOURNAL_SHA256 = "1e865f13a2af2896e1ef8c5f5e5e39e1a4240939ed616b03140b469f483c36b1"
JOURNAL_BYTES = 13376

CLAIM_MMU_10_10 = "MMU_10_10"
CLAIM_JOURNAL_BASELINE = "JOURNAL_BASELINE"

_HOST_RESULT_PASS = re.compile(r"^result=PASS\s*$", re.M)


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def old_journal_baseline_pass(text: str) -> bool:
    """Pre-A05: host header `result=PASS` in the mixed A01 OUT file is the claim."""
    return bool(_HOST_RESULT_PASS.search(text))


def old_mmu_10_10_pass(text: str, commands_got_prompt: bool = True) -> bool:
    """Pre-A05: A02 prompt-return plus A03 substring count > 0."""
    return old_a03_accept(text, commands_got_prompt=commands_got_prompt)


def classify_prior_claim(claim_id: str, text: str, evidence_sha256: str | None = None) -> tuple[str, str]:
    """Named A01–A04 harness claims cannot be a board PASS.

    Historical SHA-256 pins are REVIEW_REQUIRED even if a later classifier
    would accept a rewritten or extracted copy. Do not reinterpret.
    """
    if not text:
        return "REVIEW_REQUIRED", "EMPTY"
    if evidence_sha256 in (MMU_SHA256, JOURNAL_SHA256):
        return "REVIEW_REQUIRED", "PRIOR_HARNESS_CLAIM"
    if claim_id == CLAIM_JOURNAL_BASELINE:
        raw_ok, reason = is_raw_uart_evidence(text)
        if not raw_ok:
            return "REVIEW_REQUIRED", reason
        if old_journal_baseline_pass(text):
            return "REVIEW_REQUIRED", "HOST_RESULT_PASS"
        return "REVIEW_REQUIRED", "A01_HARNESS_CLAIM"
    if claim_id == CLAIM_MMU_10_10:
        code, reason, _ = classify_mmuiso_matrix(text)
        if code == EXIT_ACCEPT:
            return "REVIEW_REQUIRED", "DO_NOT_REINTERPRET"
        return "REVIEW_REQUIRED", reason
    return "REVIEW_REQUIRED", "UNKNOWN_CLAIM"


def _selftest() -> None:
    mmu_raw = MMU_EVIDENCE.read_bytes()
    journal_raw = JOURNAL_EVIDENCE.read_bytes()
    assert len(mmu_raw) == MMU_BYTES, len(mmu_raw)
    assert len(journal_raw) == JOURNAL_BYTES, len(journal_raw)
    assert sha256_bytes(mmu_raw) == MMU_SHA256
    assert sha256_bytes(journal_raw) == JOURNAL_SHA256

    mmu_text = mmu_raw.decode("utf-8", "replace")
    journal_text = journal_raw.decode("utf-8", "replace")

    # Old acceptance would keep both PASSes.
    assert old_journal_baseline_pass(journal_text) is True
    assert old_mmu_10_10_pass(mmu_text) is True

    raw_ok, raw_reason = is_raw_uart_evidence(journal_text)
    assert not raw_ok and raw_reason == "HOST_SUMMARY_PREFIX", (raw_ok, raw_reason)

    a03_code, a03_reason, _ = classify_mmuiso_matrix(mmu_text)
    assert a03_code != EXIT_ACCEPT and a03_reason == "DUPLICATE_OR_MISSING_DONE", (
        a03_code,
        a03_reason,
    )

    st, why = classify_prior_claim(CLAIM_JOURNAL_BASELINE, journal_text, JOURNAL_SHA256)
    assert st == "REVIEW_REQUIRED" and why == "PRIOR_HARNESS_CLAIM", (st, why)
    st, why = classify_prior_claim(CLAIM_MMU_10_10, mmu_text, MMU_SHA256)
    assert st == "REVIEW_REQUIRED" and why == "PRIOR_HARNESS_CLAIM", (st, why)

    # Negative fixture: host result=PASS, no board check — old baseline PASS.
    host_only = (
        "PhaseFS journal v1 COM12 SD baseline\n"
        "stamp=A05_REPRO\n"
        "result=PASS\n"
        "class=ON_BOARD_SOFTWARE_TEST\n"
        "gpos>\n"
    )
    assert old_journal_baseline_pass(host_only) is True
    st, why = classify_prior_claim(CLAIM_JOURNAL_BASELINE, host_only)
    assert st == "REVIEW_REQUIRED" and why == "HOST_SUMMARY_PREFIX", (st, why)
    assert "PASS" != st

    # Negative: A03-perfect extract from a later marker set cannot upgrade GOAL5.
    ordered = (
        "[ SEALED] BootSessionRoot:     fa82a350\n"
        "legal-read: PASS\ncode-read: PASS\nkernel-read: PASS\n"
        "kernel-write: PASS\npeer-read: PASS\npeer-write: PASS\n"
        "mmio-write: PASS\ncode-write: PASS\nexecute-data: PASS\n"
        "stack-overflow: PASS\nmmuiso matrix done\n"
    )
    code, reason, _ = classify_mmuiso_matrix(ordered)
    assert code == EXIT_ACCEPT and reason == "ORDERED_SINGLE_RUN", (code, reason)
    st, why = classify_prior_claim(CLAIM_MMU_10_10, ordered)
    assert st == "REVIEW_REQUIRED" and why == "DO_NOT_REINTERPRET", (st, why)
    st, why = classify_prior_claim(CLAIM_MMU_10_10, ordered, MMU_SHA256)
    assert st == "REVIEW_REQUIRED" and why == "PRIOR_HARNESS_CLAIM", (st, why)

    audit = AUDIT_DOC.read_text(encoding="utf-8")
    status = STATUS_DOC.read_text(encoding="utf-8")
    assert MMU_SHA256 in audit
    assert "DUPLICATE_OR_MISSING_DONE" in audit or "23cea7b1" in audit.lower()
    assert "b15168ef9ba8396717cb98b8d0defeadf1e00990639f82726edbe57840b6547a" in audit
    assert "222341" in status
    assert "1e865f13a2af2896e1ef8c5f5e5e39e1a4240939ed616b03140b469f483c36b1" in status
    assert "0de6542114d5824ec4036c55078a5cca127fd40f2c52c5fa9bc4ed5bdb31a3f5" in status
    assert "HOST_SUMMARY_PREFIX" in status

    print("a05_claim_review selftest: ok")


def main(argv: list[str]) -> int:
    if len(argv) >= 2 and argv[1] == "--selftest":
        _selftest()
        return 0
    print("usage: a05_claim_review.py --selftest", file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
