"""Replace the last (application) partition of a proven Zynq-7000 BOOT.BIN.

Keeps FSBL + bitstream from the gold image. Layout and checksum were
measured from PZ7020_G1P_PATH1_G64/BOOT.BIN (bootgen output).
"""
from __future__ import annotations

import hashlib
import struct
import sys
from pathlib import Path


def u32(buf: bytes, off: int) -> int:
    return int.from_bytes(buf[off : off + 4], "little")


def load_blob(elf: bytes) -> tuple[int, int, bytes]:
    if elf[:4] != b"\x7fELF":
        raise SystemExit("not ELF")
    e_entry = u32(elf, 24)
    e_phoff = u32(elf, 28)
    e_phentsize = int.from_bytes(elf[42:44], "little")
    e_phnum = int.from_bytes(elf[44:46], "little")
    segs: list[tuple[int, int, int]] = []
    for i in range(e_phnum):
        o = e_phoff + i * e_phentsize
        p_type, p_off, _vaddr, p_paddr, p_filesz, _memsz, _flags, _align = struct.unpack_from(
            "<IIIIIIII", elf, o
        )
        if p_type == 1 and p_filesz > 0:
            segs.append((p_paddr, p_off, p_filesz))
    if not segs:
        raise SystemExit("no PT_LOAD")
    base = min(s[0] for s in segs)
    end = max(s[0] + s[2] for s in segs)
    blob = bytearray(end - base)
    for paddr, off, filesz in segs:
        blob[paddr - base : paddr - base + filesz] = elf[off : off + filesz]
    while len(blob) % 4:
        blob.append(0)
    return e_entry, base, bytes(blob)


def pack(boot: bytearray, elf: bytes) -> bytes:
    entry, dest, blob = load_blob(elf)
    app_off = 0xFF250 * 4
    ph = 0xD00
    nwords = len(blob) // 4
    words = [u32(bytes(boot[ph : ph + 64]), i) for i in range(0, 64, 4)]
    words[0] = nwords
    words[1] = nwords
    words[2] = nwords
    words[3] = dest
    words[4] = entry
    s = sum(words[:15])
    words[15] = (~s) & 0xFFFFFFFF
    for i, w in enumerate(words):
        boot[ph + i * 4 : ph + i * 4 + 4] = w.to_bytes(4, "little")
    return bytes(boot[:app_off] + blob)


def main() -> None:
    if len(sys.argv) != 4:
        raise SystemExit("usage: pack_zynq_app_into_boot.py GOLD_BOOT.BIN APP.elf OUT_BOOT.BIN")
    gold, elf_path, out = map(Path, sys.argv[1:])
    boot = bytearray(gold.read_bytes())
    elf = elf_path.read_bytes()
    new = pack(boot, elf)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_bytes(new)
    print(hashlib.sha256(new).hexdigest(), out, len(new))


if __name__ == "__main__":
    main()
