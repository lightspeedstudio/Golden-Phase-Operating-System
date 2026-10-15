#!/usr/bin/env python3
# Copyright LightSpeed Studio LLC
# SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
"""Validate LightSpeed Studio LLC Stage 1 summary docs. No COM12. No SD write."""

from __future__ import annotations

import hashlib
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LEGAL = "LightSpeed Studio LLC"
BAD_NAMES = ("Light Speed Studios", "LightSpeed Studios")
DOC_GLOB = "docs/boot/LIGHTSPEED_STAGE1_*.md"
INDEX = ROOT / "docs/boot/README.md"
DEFER = ROOT / "docs/boot/DEFERRED_PRODUCTION_SECURITY_HARDWARE.md"

ARTIFACTS = [
    (
        "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS12_20260820/com12_sys12.uart",
        "6236336b06b9b4cb70fc098f8616bb91b7b790864dd53e0bc5d73b90bb5cf9d5",
        7010,
    ),
    (
        "fpga/zynq/release/PZ7020_STAGE1_PLAINTEXT/BOOT.BIN",
        "738c6a0155d9558c37d51fffe11bc99236c4c29242caa07fa860756f47d24352",
        6016212,
    ),
    (
        "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS12_20260820/BOOT_SYS12_PLAINTEXT.BIN",
        "738c6a0155d9558c37d51fffe11bc99236c4c29242caa07fa860756f47d24352",
        6016212,
    ),
    (
        "docs/boot/sys10a/sys10a_plat.elf",
        "cbc4e37ad3a8444541db886a040a42dc0c94c9a09102bffb3b51f5280ee26854",
        50292,
    ),
    (
        "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS10A_20260820/com12_sys10a.uart",
        "ceb02585476e6b316f94c01c720e4f385ba1f6d86aaf2b1c9a3a67dd3294c64d",
        7107,
    ),
    (
        "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS09_20260820/com12_sys09.uart",
        "391218176cef09b7d642b71f5cca9990dec2d91b95cc73c9879185d9dfa0219d",
        6994,
    ),
    (
        "fpga/zynq/evidence/kairos_zynq_boot/com12_path1_g64_combined_20260814_174430.txt",
        "bf3ca7940c61e22455276b26d218bb7bd44203859e2362a26e263a152e102800",
        7475,
    ),
    (
        "fpga/zynq/release/PZ7020_G1P_PATH1_G64/BOOT.BIN",
        "7da748547d5016191643ddc4df555a8cf02c1d8adee3235f1dde766166ea0b85",
        6114836,
    ),
    (
        "fpga/zynq/release/PZ7020_G1P_PATH1_G64/system.bit",
        "d19f08d20cac85ea0e33c41a8a552166f5a9176e16977f09aa64d17e68135829",
        4045769,
    ),
    (
        "fpga/zynq/release/PZ7020_G1P_PATH1_G64/kairos_zynq.elf",
        "447f9fbdec6bbdb03227c8620baa854f3a141d52ec71452251e2f63e1852eaf8",
        2475164,
    ),
    (
        "fpga/zynq/release/PZ7020_G1P_BRINGUP_V2/zynq_fsbl.elf",
        "8b71aa707fd9e200a89cd2b966cf3d3ec26ed0350495718a51a9f8ec259f7874",
        577272,
    ),
    (
        "zynq_stage1/src/main.rs",
        "6637678d0e01b007caf3b5e150ed37e48ed5af6f55cc009a3b16e0d5c4ebd67d",
        9580,
    ),
    (
        "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS03_20260819/com12_sys03.uart",
        "8fcdc3257f3449a9114594474d0e34d8e71540348081da82336f819f132f0dee",
        112,
    ),
    (
        "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS06_20260820/com12_sys06.uart",
        "ad6b0106577972effe75e0b6597a980b55242cedbf2f650c8b4cd1b63814c95c",
        284,
    ),
    (
        "docs/boot/sys10/sys09_plat.elf.board_passed",
        "08502fd5fd9f9a5c16f333571a1e9d6f9970bb1d4618ae87a3683e0d4f77c082",
        34072,
    ),
    (
        "zynq_stage1/owned/build/BOOT_RECOVERY_PLAINTEXT.BIN",
        "46bb693c123df147609cc26f765fa68f7bc935edbea88d5a01f5976c05c713fb",
        5992148,
    ),
]

DOCS = [
    "docs/boot/README.md",
    "docs/boot/LIGHTSPEED_STAGE1_EXECUTIVE_SUMMARY.md",
    "docs/boot/LIGHTSPEED_STAGE1_TECHNICAL_OVERVIEW.md",
    "docs/boot/LIGHTSPEED_STAGE1_SYSTEM_LEDGER.md",
    "docs/boot/LIGHTSPEED_STAGE1_EVIDENCE_INDEX.md",
    "docs/boot/LIGHTSPEED_STAGE1_OWNERSHIP.md",
    "docs/boot/LIGHTSPEED_STAGE1_SECURITY_STATUS.md",
    "docs/boot/LIGHTSPEED_STAGE1_BOOT_PERFORMANCE.md",
    "docs/boot/LIGHTSPEED_STAGE1_RELEASE_STATUS.md",
    "docs/boot/DEFERRED_PRODUCTION_SECURITY_HARDWARE.md",
    "docs/boot/SYS12_RECOVERY_REFERENCE.md",
]


def sha256_file(p: Path) -> str:
    h = hashlib.sha256()
    with p.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def main() -> int:
    errors: list[str] = []
    for rel, expect, n in ARTIFACTS:
        p = ROOT / rel
        if not p.is_file():
            errors.append(f"missing {rel}")
            continue
        got_n = p.stat().st_size
        got = sha256_file(p)
        if got_n != n:
            errors.append(f"size {rel} {got_n} != {n}")
        if got != expect:
            errors.append(f"hash {rel} {got} != {expect}")

    texts: list[str] = []
    for rel in DOCS:
        p = ROOT / rel
        if not p.is_file():
            errors.append(f"missing {rel}")
            continue
        t = p.read_text(encoding="utf-8")
        texts.append(t)
        if LEGAL not in t:
            errors.append(f"legal name missing in {rel}")
        for bad in BAD_NAMES:
            if bad in t:
                errors.append(f"forbidden company spelling {bad!r} in {rel}")
        if "-----BEGIN" in t or "PRIVATE KEY" in t:
            errors.append(f"key material in {rel}")
        if "is an 8 KiB PASS" in t or "8192 PASS" in t:
            errors.append(f"8 KiB PASS claim in {rel}")
        lower = t.lower()
        if "bootrom rsa is eFuse-enforced".lower() in lower:
            errors.append(f"BootROM RSA enforcement claim in {rel}")
        if "bbram is power-retained" in lower or "cold-power bbram retention is proven" in lower:
            errors.append(f"BBRAM retention claim in {rel}")

    blob = "\n".join(texts)
    for needle in (
        "6236336b06b9b4cb70fc098f8616bb91b7b790864dd53e0bc5d73b90bb5cf9d5",
        "738c6a0155d9558c37d51fffe11bc99236c4c29242caa07fa860756f47d24352",
        "SYS10_OVERSIZE",
        "DEFERRED_CUSTOM_HARDWARE",
        "PL PASS",
        "JUMP 00100060",
        "gpos>",
    ):
        if needle not in blob:
            errors.append(f"summary set missing {needle}")

    if not INDEX.is_file():
        errors.append("missing docs/boot/README.md")
    else:
        idx = INDEX.read_text(encoding="utf-8")
        for name in (
            "LIGHTSPEED_STAGE1_EXECUTIVE_SUMMARY.md",
            "LIGHTSPEED_STAGE1_TECHNICAL_OVERVIEW.md",
            "LIGHTSPEED_STAGE1_SYSTEM_LEDGER.md",
            "LIGHTSPEED_STAGE1_EVIDENCE_INDEX.md",
            "LIGHTSPEED_STAGE1_OWNERSHIP.md",
            "LIGHTSPEED_STAGE1_SECURITY_STATUS.md",
            "LIGHTSPEED_STAGE1_BOOT_PERFORMANCE.md",
            "LIGHTSPEED_STAGE1_RELEASE_STATUS.md",
            "DEFERRED_PRODUCTION_SECURITY_HARDWARE.md",
        ):
            if name not in idx:
                errors.append(f"index missing link {name}")

    if errors:
        print("FAIL")
        for e in errors:
            print(e)
        return 1
    print("PASS", len(ARTIFACTS), "artifacts", len(DOCS), "docs")
    return 0


if __name__ == "__main__":
    sys.exit(main())
