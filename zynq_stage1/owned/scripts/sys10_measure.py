#!/usr/bin/env python3
# Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
# SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
"""Host measurement of board-passed System 9 Stage 1. Does not rebuild or flash."""

from __future__ import annotations

import hashlib
import struct
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
EV = ROOT / "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS10_20260820"
DOCS = ROOT / "docs/boot"
ELF = DOCS / "sys10/sys09_plat.elf.board_passed"
BIN = DOCS / "sys10/sys09_plat.bin.board_passed"
BOOT = EV / "BOOT_SYS09_HANDOFF.BIN.board_passed"
MAP = EV / "sys03_plat.map.at_sys09_lock"
NM = DOCS / "sys10/nm_board_passed.txt"
UART = ROOT / "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS09_20260820/com12_sys09.uart"

BOARD_BOOT = "eb2f34c3052f9c1a17b0ab34d31385decb0a3cc1c9c75fabacce97e9e63d7b27"
BOARD_UART = "391218176cef09b7d642b71f5cca9990dec2d91b95cc73c9879185d9dfa0219d"
BOARD_ELF = "08502fd5fd9f9a5c16f333571a1e9d6f9970bb1d4618ae87a3683e0d4f77c082"
GATE = 8192
PT_LOAD = 1


def sha256(p: Path) -> str:
    return hashlib.sha256(p.read_bytes()).hexdigest()


def elf_pt_loads(data: bytes) -> list[dict]:
    if data[:4] != b"\x7fELF" or data[4] != 1 or data[5] != 1:
        raise SystemExit("not ELF32 LE")
    phoff = struct.unpack_from("<I", data, 0x1C)[0]
    phentsize = struct.unpack_from("<H", data, 0x2A)[0]
    phnum = struct.unpack_from("<H", data, 0x2C)[0]
    entry = struct.unpack_from("<I", data, 0x18)[0]
    loads = []
    for i in range(phnum):
        o = phoff + i * phentsize
        ptype, off, vaddr, paddr, filesz, memsz, flags, align = struct.unpack_from(
            "<IIIIIIII", data, o
        )
        rec = {
            "i": i,
            "type": ptype,
            "offset": off,
            "vaddr": vaddr,
            "paddr": paddr,
            "filesz": filesz,
            "memsz": memsz,
            "flags": flags,
            "align": align,
        }
        if ptype == PT_LOAD:
            rec["payload"] = data[off : off + filesz]
        loads.append(rec)
    return entry, loads


def parse_nm(text: str) -> list[tuple[int, int, str, str]]:
    rows = []
    for line in text.splitlines():
        line = line.strip()
        if not line:
            continue
        parts = line.split()
        if len(parts) == 3 and len(parts[0]) == 8:
            addr = int(parts[0], 16)
            kind = parts[1]
            name = parts[2]
            rows.append((addr, 0, kind, name))
        elif len(parts) >= 4:
            addr = int(parts[0], 16)
            size = int(parts[1], 16)
            kind = parts[2]
            name = " ".join(parts[3:])
            rows.append((addr, size, kind, name))
    return rows


def classify_map(name: str) -> str:
    n = name.replace("\\", "/")
    base = n.rsplit("/", 1)[-1]
    low = n.lower()
    if "zynq_fsbl" in low or "xilinx" in low or "bootgen" in low or "kairos" in low:
        return "unknown"
    if "compiler_builtins" in n or "aeabi_div0" in n or "udivsi3" in n:
        return "compiler_builtins"
    if "libcore-" in base or "/libcore" in n or base.startswith("core"):
        return "rust_core"
    if "zynq_stage1_owned" in n or "sys09_plat" in n:
        return "lightspeed_owned"
    if n.endswith(".ld") or "linker script" in low:
        return "linker_runtime"
    return "unknown"


def module_of(name: str) -> str:
    if name.startswith("zynq_stage1_owned::"):
        rest = name[len("zynq_stage1_owned::") :]
        return "owned:" + rest.split("::")[0]
    if name.startswith("core::"):
        return "core"
    if name.startswith("compiler_builtins::") or name.startswith("__aeabi") or name in (
        "__udivsi3",
        "__udivmoddi4",
        "memcmp",
        "__aeabi_idiv0",
    ):
        return "compiler_builtins"
    if name in ("_start", "hang", "rust_main", "sys09_plat::KEEP_RUST_MAIN"):
        return "owned:entry"
    return "unidentified"


def main() -> None:
    elf = ELF.read_bytes()
    assert sha256(ELF) == BOARD_ELF
    assert sha256(UART) == BOARD_UART
    entry, phs = elf_pt_loads(elf)
    loads = [p for p in phs if p["type"] == PT_LOAD]
    assert len(loads) == 1
    L = loads[0]
    payload = L["payload"]
    assert hashlib.sha256(payload).hexdigest() == sha256(BIN)
    src = 0x1700
    data_words = enc_words = 4822
    if BOOT.exists():
        assert sha256(BOOT) == BOARD_BOOT
        boot = BOOT.read_bytes()
        src = struct.unpack_from("<I", boot, 0x30)[0]
        img = struct.unpack_from("<I", boot, 0x34)[0]
        assert boot[src : src + img] == payload
        ph0 = struct.unpack_from("<III", boot, 0xC80)
        enc_words, data_words = ph0[0], ph0[1]
    _ = entry

    nm_rows = parse_nm(NM.read_text(encoding="ascii", errors="replace"))
    sized = [(a, s, k, n) for a, s, k, n in nm_rows if s]
    sized.sort(key=lambda r: (-r[1], r[0], r[3]))
    by_addr: dict[int, int] = {}
    for a, s, k, n in sized:
        by_addr[a] = max(by_addr.get(a, 0), s)
    totals: dict[str, int] = defaultdict(int)
    seen_addr: set[int] = set()
    for a, s, k, n in sized:
        if a in seen_addr:
            continue
        seen_addr.add(a)
        totals[module_of(n)] += by_addr[a]
    sum_sized = sum(by_addr.values())
    unidentified = L["filesz"] - sum_sized

    members = []
    if MAP.exists():
        for line in MAP.read_text(encoding="utf-8", errors="replace").splitlines():
            if ".rlib(" in line or ".rcgu.o" in line or line.strip().endswith(".o)"):
                members.append(line.strip())
    classes = defaultdict(int)
    unknown = []
    for m in members:
        c = classify_map(m)
        classes[c] += 1
        if c == "unknown":
            unknown.append(m)

    filesz_sum = sum(p["filesz"] for p in loads)
    memsz_sum = sum(p["memsz"] for p in loads)
    stack_top = 0x6B60
    ocm_span = stack_top
    status = "SYS10_INCOMPLETE_LINK_SET"

    hashes = []
    paths = [
        ELF,
        BIN,
        BOOT,
        UART,
        MAP,
        ROOT / "zynq_stage1/src/main.rs",
        ROOT / "zynq_stage1/owned/linker.ld",
        ROOT / "fpga/zynq/release/PZ7020_G1P_PATH1_G64/system.bit",
        ROOT / "fpga/zynq/release/PZ7020_G1P_PATH1_G64/kairos_zynq.elf",
        ROOT / "fpga/zynq/release/PZ7020_G1P_PATH1_G64/BOOT.BIN",
        ROOT / "docs/boot/sys10/sys09_plat.elf.board_passed",
        ROOT / "docs/boot/sys10/sys09_plat.bin.board_passed",
        ROOT / "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS09_20260820/HASHES_PASS.txt",
        ROOT / "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS09_20260820/CLASSIFIER_PASS.txt",
    ]
    for p in paths:
        if not p.exists():
            continue
        rel = p.relative_to(ROOT).as_posix()
        hashes.append(f"{rel}\t{sha256(p)}\t{p.stat().st_size}")

    DOCS.mkdir(parents=True, exist_ok=True)
    (DOCS / "SYS10_ARTIFACT_HASHES.txt").write_text(
        "\n".join(
            [
                "# System 10 artifact hashes. BOARD_PASSED_ARTIFACT unless labeled CURRENT_REBUILD.",
                f"status\t{status}",
                f"board_passed_boot\t{BOARD_BOOT}\t6005972",
                f"board_passed_uart\t{BOARD_UART}\t6994",
                f"board_passed_elf\t{BOARD_ELF}\t{ELF.stat().st_size}",
                f"utility_entry\t0x00100060",
                f"gate_bytes\t{GATE}",
                f"pt_load_filesz_sum\t{filesz_sum}",
                f"pt_load_memsz_sum\t{memsz_sum}",
                f"ocm_span\t{ocm_span}",
                "",
                "# path\tsha256\tbytes",
                *hashes,
                "",
            ]
        ),
        encoding="ascii",
        newline="\n",
    )

    sym_lines = ["# nm --print-size --size-sort, largest first. BOARD_PASSED_ARTIFACT sys09_plat.elf"]
    for a, s, k, n in sized:
        sym_lines.append(f"{s:8d}  {a:08x}  {k}  {n}  [{module_of(n)}]")
    sym_lines.append(f"# sum_nm_sizes {sum_sized}")
    sym_lines.append(f"# pt_load_filesz {L['filesz']}")
    sym_lines.append(f"# unidentified_or_unsized {unidentified}")
    (DOCS / "SYS10_SYMBOL_SIZES.txt").write_text("\n".join(sym_lines) + "\n", encoding="ascii", newline="\n")

    map_lines = [
        "# Linker map summary. BOARD_PASSED_ARTIFACT map file is sys03_plat.map.at_sys09_lock",
        "# (owned/.cargo/config.toml always writes build/sys03_plat.map). Product ELF matched rebuild;",
        "# CURRENT_REBUILD map first differs at CARGO_TARGET_DIR path (owned/target vs sandbox cache).",
        f"map_board_passed_bytes\t{MAP.stat().st_size}",
        f"map_board_passed_sha256\t{sha256(MAP)}",
        "archive_member_classes:",
    ]
    for k in sorted(classes):
        map_lines.append(f"  {k}\t{classes[k]}")
    map_lines.append(f"unknown_count\t{len(unknown)}")
    for u in unknown[:20]:
        map_lines.append(f"UNKNOWN\t{u[:200]}")
    map_lines.append("module_nm_totals:")
    for k in sorted(totals, key=lambda x: -totals[x]):
        map_lines.append(f"  {k}\t{totals[k]}")
    (DOCS / "SYS10_LINK_MAP_SUMMARY.txt").write_text("\n".join(map_lines) + "\n", encoding="ascii", newline="\n")

    print("status", status)
    print("filesz", filesz_sum, "memsz", memsz_sum, "ocm", ocm_span)
    print("nm_sum", sum_sized, "unidentified", unidentified)
    print("unknown_map", len(unknown))
    print("entry", hex(entry), "p0_off", hex(src), "data_words", data_words, "enc_words", enc_words)


if __name__ == "__main__":
    main()
