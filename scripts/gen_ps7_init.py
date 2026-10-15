#!/usr/bin/env python3
"""Generate src/board/zynq7000/ps7_init_data.rs from the sealed .xsa.

Regeneration is explicit and happens ONLY when the .xsa changes:

    python scripts/gen_ps7_init.py \
        [fpga/zynq/release/PZ7020_G1P_BRINGUP_V2/PZ7020_G1P_BRINGUP_V2.xsa] \
        [src/board/zynq7000/ps7_init_data.rs]

The normal OS build consumes the checked-in Rust file and does not require
Python, Vivado, or the .xsa.

The complete PS7 initialization operation stream is preserved in its original
order — masked writes, polls, and delays — in the exact ps7_init.h encoding
((opcode << 4) | nargs, then args). Unknown operation types are a hard error.
Only the silicon-3.0 arrays are emitted (ps7_init() run-time dispatch for
1.0/2.0 silicon is intentionally not carried over; stage-0 checks and reports
the live silicon version).
"""

import hashlib
import re
import struct
import sys
import zipfile
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
DEFAULT_XSA = REPO / "fpga/zynq/release/PZ7020_G1P_BRINGUP_V2/PZ7020_G1P_BRINGUP_V2.xsa"
DEFAULT_OUT = REPO / "src/board/zynq7000/ps7_init_data.rs"

# ps7_init.h encoding
OPCODES = {
    "EMIT_EXIT": (0, 0),
    "EMIT_CLEAR": (1, 1),
    "EMIT_WRITE": (2, 2),
    "EMIT_MASKWRITE": (3, 3),
    "EMIT_MASKPOLL": (4, 2),
    "EMIT_MASKDELAY": (5, 2),
}

ARRAYS = [
    ("ps7_mio_init_data_3_0", "PS7_MIO_INIT"),
    ("ps7_pll_init_data_3_0", "PS7_PLL_INIT"),
    ("ps7_clock_init_data_3_0", "PS7_CLOCK_INIT"),
    ("ps7_ddr_init_data_3_0", "PS7_DDR_INIT"),
    ("ps7_peripherals_init_data_3_0", "PS7_PERIPHERALS_INIT"),
    ("ps7_post_config_3_0", "PS7_POST_CONFIG"),
]


def strip_comments(text: str) -> str:
    text = re.sub(r"/\*.*?\*/", " ", text, flags=re.S)
    text = re.sub(r"//[^\n]*", " ", text)
    return text


def parse_int(tok: str) -> int:
    tok = tok.strip().rstrip("Uu")
    return int(tok, 0)


def parse_array(source: str, c_name: str) -> list[tuple[str, list[int]]]:
    m = re.search(
        r"unsigned long\s+" + re.escape(c_name) + r"\[\]\s*=\s*\{(.*?)\};",
        source,
        flags=re.S,
    )
    if not m:
        raise SystemExit(f"ERROR: array {c_name} not found in ps7_init.c")
    body = strip_comments(m.group(1))
    ops: list[tuple[str, list[int]]] = []
    pos = 0
    for call in re.finditer(r"(EMIT_[A-Z_]+)\s*\(([^)]*)\)\s*,?", body):
        # Anything between calls other than whitespace is unexpected.
        gap = body[pos : call.start()].strip()
        if gap:
            raise SystemExit(f"ERROR: unexpected token in {c_name}: {gap!r}")
        pos = call.end()
        name = call.group(1)
        if name not in OPCODES:
            raise SystemExit(f"ERROR: unknown operation {name} in {c_name}")
        args_src = call.group(2).strip()
        args = [parse_int(a) for a in args_src.split(",")] if args_src else []
        _, nargs = OPCODES[name]
        if len(args) != nargs:
            raise SystemExit(
                f"ERROR: {name} in {c_name} has {len(args)} args, expected {nargs}"
            )
        ops.append((name, args))
    tail = body[pos:].strip()
    if tail:
        raise SystemExit(f"ERROR: unexpected trailing token in {c_name}: {tail!r}")
    if not ops or ops[-1][0] != "EMIT_EXIT":
        raise SystemExit(f"ERROR: {c_name} does not end with EMIT_EXIT")
    return ops


def encode(ops: list[tuple[str, list[int]]]) -> list[int]:
    words: list[int] = []
    for name, args in ops:
        opcode, nargs = OPCODES[name]
        words.append((opcode << 4) | nargs)
        words.extend(args)
    return words


def main() -> None:
    xsa_path = Path(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT_XSA
    out_path = Path(sys.argv[2]) if len(sys.argv) > 2 else DEFAULT_OUT

    xsa_bytes = xsa_path.read_bytes()
    xsa_sha = hashlib.sha256(xsa_bytes).hexdigest()
    with zipfile.ZipFile(xsa_path) as z:
        source = z.read("ps7_init.c").decode("utf-8", errors="replace")
    src_sha = hashlib.sha256(source.encode()).hexdigest()

    m = re.search(r"#define\s+APU_FREQ\s+(\d+)", source)
    apu_freq = int(m.group(1)) if m else None
    if apu_freq is None:
        # APU_FREQ lives in ps7_init.h inside the same .xsa
        with zipfile.ZipFile(xsa_path) as z:
            header = z.read("ps7_init.h").decode("utf-8", errors="replace")
        m = re.search(r"#define\s+APU_FREQ\s+(\d+)", header)
        if not m:
            raise SystemExit("ERROR: APU_FREQ not found in ps7_init.c/.h")
        apu_freq = int(m.group(1))

    encoded: dict[str, list[int]] = {}
    op_counts: dict[str, int] = {}
    stream_hash = hashlib.sha256()
    for c_name, rust_name in ARRAYS:
        ops = parse_array(source, c_name)
        words = encode(ops)
        encoded[rust_name] = words
        op_counts[rust_name] = len(ops)
        for w in words:
            stream_hash.update(struct.pack("<I", w))
    data_sha = stream_hash.hexdigest()

    lines: list[str] = []
    lines.append("//! GENERATED FILE — DO NOT EDIT BY HAND.")
    lines.append("//!")
    lines.append("//! PS7 initialization operation streams for the PZ7020-SL-C, extracted")
    lines.append("//! verbatim (order preserved) from the sealed hardware export.")
    lines.append("//!")
    lines.append(f"//! source .xsa:  {xsa_path.as_posix()}")
    lines.append(f"//! .xsa sha256:       {xsa_sha}")
    lines.append(f"//! ps7_init.c sha256: {src_sha}")
    lines.append(f"//! encoded-stream sha256 (LE u32 words, arrays in file order): {data_sha}")
    lines.append("//!")
    lines.append("//! Regenerate ONLY when the .xsa changes:")
    lines.append("//!     python scripts/gen_ps7_init.py")
    lines.append("//!")
    lines.append("//! Encoding (ps7_init.h): word0 = (opcode << 4) | nargs, then the args.")
    lines.append("//! Opcodes: 0 EXIT, 1 CLEAR, 2 WRITE, 3 MASKWRITE, 4 MASKPOLL, 5 MASKDELAY.")
    lines.append("//! Silicon-3.0 arrays only; stage-0 reports the live silicon version.")
    lines.append("")
    lines.append("/// CPU (APU) clock in Hz; the SCU global timer runs at APU/2 (CPU_3x2x).")
    lines.append(f"pub const APU_FREQ_HZ: u32 = {apu_freq};")
    for c_name, rust_name in ARRAYS:
        words = encoded[rust_name]
        lines.append("")
        lines.append(f"/// `{c_name}` — {op_counts[rust_name]} ops, {len(words)} words.")
        lines.append(f"pub static {rust_name}: &[u32] = &[")
        # One op per line for reviewability.
        i = 0
        while i < len(words):
            nargs = words[i] & 0xF
            chunk = words[i : i + 1 + nargs]
            lines.append("    " + ", ".join(f"0x{w:08X}" for w in chunk) + ",")
            i += 1 + nargs
        lines.append("];")
    lines.append("")

    out_path.write_text("\n".join(lines), encoding="utf-8", newline="\n")
    total_ops = sum(op_counts.values())
    print(f"wrote {out_path} ({total_ops} ops)")
    for _, rust_name in ARRAYS:
        print(f"  {rust_name}: {op_counts[rust_name]} ops")
    print(f"  .xsa sha256:   {xsa_sha}")
    print(f"  stream sha256: {data_sha}")


if __name__ == "__main__":
    main()
