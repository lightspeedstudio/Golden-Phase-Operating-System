#!/usr/bin/env python3
# Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
"""Emit named Stage-1 PS7 steps from the PZ7020 board register streams."""
from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "src" / "board" / "zynq7000" / "ps7_init_data.rs"
OUT = ROOT / "src" / "board" / "zynq7000" / "stage1" / "ps7.rs"

DDRC = {
    0xF8006000: "ddrc_ctrl",
    0xF8006004: "Two_rank_cfg",
    0xF8006008: "HPR_reg",
    0xF800600C: "LPR_reg",
    0xF8006010: "WR_reg",
    0xF8006014: "DRAM_param_reg0",
    0xF8006018: "DRAM_param_reg1",
    0xF800601C: "DRAM_param_reg2",
    0xF8006020: "DRAM_param_reg3",
    0xF8006024: "DRAM_param_reg4",
    0xF8006028: "DRAM_init_param",
    0xF800602C: "DRAM_EMR_reg",
    0xF8006030: "DRAM_EMR_MR_reg",
    0xF8006034: "DRAM_burst8_rdwr",
    0xF8006038: "DRAM_disable_DQ",
    0xF800603C: "DRAM_addr_map_bank",
    0xF8006040: "DRAM_addr_map_col",
    0xF8006044: "DRAM_addr_map_row",
    0xF8006048: "DRAM_ODT_reg",
    0xF8006050: "PHY_cmd_timeout",
    0xF8006054: "DLL_calib",
    0xF8000B74: "ddrc_status_poll",
}


def array_words(text: str, name: str) -> list[int]:
    m = re.search(
        rf"pub static {name}: &\[u32\] = &\[(.*?)\];",
        text,
        re.S,
    )
    if not m:
        raise SystemExit(f"missing {name}")
    return [int(x, 16) for x in re.findall(r"0x[0-9A-Fa-f]+", m.group(1))]


def decode(words: list[int]) -> list[tuple]:
    i = 0
    ops: list[tuple] = []
    while i < len(words):
        w = words[i]
        op = w >> 4
        n = w & 0xF
        args = words[i + 1 : i + 1 + n]
        i += 1 + n
        if op == 0:
            ops.append(("Exit",))
            break
        if op == 1:
            ops.append(("Clear", args[0]))
        elif op == 2:
            ops.append(("Write", args[0], args[1]))
        elif op == 3:
            ops.append(("MaskWrite", args[0], args[1], args[2]))
        elif op == 4:
            ops.append(("Poll", args[0], args[1]))
        elif op == 5:
            ops.append(("DelayMs", args[1]))
        else:
            raise SystemExit(f"bad opcode {op}")
    return ops


def rust_ops(ops: list[tuple]) -> str:
    lines = []
    for op in ops:
        if op[0] == "Exit":
            continue
        if op[0] == "Write" and op[1] == 0xF8000008:
            lines.append("        Step::Unlock,")
        elif op[0] == "Write" and op[1] == 0xF8000004:
            lines.append("        Step::Lock,")
        elif op[0] == "Write":
            lines.append(f"        Step::Write(0x{op[1]:08X}, 0x{op[2]:08X}),")
        elif op[0] == "Clear":
            lines.append(f"        Step::Write(0x{op[1]:08X}, 0),")
        elif op[0] == "MaskWrite":
            c = ""
            if op[1] in DDRC:
                c = f" // {DDRC[op[1]]}"
            elif 0xF8000700 <= op[1] <= 0xF80007D4:
                pin = (op[1] - 0xF8000700) // 4
                c = f" // MIO_PIN_{pin:02d}"
            elif op[1] == 0xF8000100:
                c = " // ARM_PLL_CTRL"
            elif op[1] == 0xF8000104:
                c = " // DDR_PLL_CTRL"
            elif op[1] == 0xF8000108:
                c = " // IO_PLL_CTRL"
            elif op[1] == 0xF800010C:
                c = " // PLL_STATUS"
            elif op[1] == 0xF8000110:
                c = " // ARM_PLL_CFG"
            elif op[1] == 0xF8000114:
                c = " // DDR_PLL_CFG"
            elif op[1] == 0xF8000118:
                c = " // IO_PLL_CFG"
            elif op[1] == 0xF8000120:
                c = " // ARM_CLK_CTRL"
            elif op[1] == 0xF8000124:
                c = " // DDR_CLK_CTRL"
            elif op[1] == 0xF8000140:
                c = " // GEM0_CLK_CTRL"
            elif op[1] == 0xF8000150:
                c = " // SDIO_CLK_CTRL"
            elif op[1] == 0xF8000168:
                c = " // UART_CLK_CTRL"
            elif op[1] == 0xE0000000:
                c = " // UART0 CR"
            lines.append(
                f"        Step::MaskWrite(0x{op[1]:08X}, 0x{op[2]:08X}, 0x{op[3]:08X}),{c}"
            )
        elif op[0] == "Poll":
            lines.append(f"        Step::Poll(0x{op[1]:08X}, 0x{op[2]:08X}),")
        elif op[0] == "DelayMs":
            lines.append(f"        Step::DelayMs({op[1]}),")
    return "\n".join(lines)


def fn(name: str, doc: str, ops: list[tuple]) -> str:
    return (
        f"/// {doc}\n"
        f"unsafe fn {name}() -> Result<(), u32> {{\n"
        f"    const STEPS: &[Step] = &[\n"
        f"{rust_ops(ops)}\n"
        f"    ];\n"
        f"    run(STEPS)\n"
        f"}}\n"
    )


def main() -> None:
    text = SRC.read_text(encoding="utf-8")
    chunks = [
        (
            "mio",
            "MIO pin mux + SLCR analog (UG585 Ch. 2).",
            decode(array_words(text, "PS7_MIO_INIT")),
        ),
        (
            "pll",
            "ARM/DDR/IO PLL: bypass, FBDIV, reset, lock, un-bypass (UG585).",
            decode(array_words(text, "PS7_PLL_INIT")),
        ),
        (
            "clocks",
            "UART/SD/GEM and APER clocks (UG585 clock control).",
            decode(array_words(text, "PS7_CLOCK_INIT")),
        ),
        (
            "ddr",
            "DDRC + PHY for MT41K256M16-125, 16-bit, ~533 MHz.",
            decode(array_words(text, "PS7_DDR_INIT")),
        ),
        (
            "peripherals",
            "UART0, GPIO bank, DevCfg bits used before PCAP.",
            decode(array_words(text, "PS7_PERIPHERALS_INIT")),
        ),
    ]
    body = "\n".join(fn(*c) for c in chunks)
    out = f"""// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! PZ7020-SL-C PS7 bring-up as named UG585 MMIO steps.
//!
//! Values are this PCB (33.333 MHz crystal, MT41K256M16). This file is not
//! Xilinx FSBL source.

use super::mmio;

#[derive(Clone, Copy)]
enum Step {{
    Unlock,
    Lock,
    Write(u32, u32),
    MaskWrite(u32, u32, u32),
    Poll(u32, u32),
    DelayMs(u32),
}}

fn run(steps: &[Step]) -> Result<(), u32> {{
    unsafe {{
        for s in steps {{
            match *s {{
                Step::Unlock => mmio::slcr_unlock(),
                Step::Lock => mmio::slcr_lock(),
                Step::Write(a, v) => mmio::write(a, v),
                Step::MaskWrite(a, m, v) => mmio::mask_write(a, m, v),
                Step::Poll(a, m) => mmio::poll(a, m)?,
                Step::DelayMs(ms) => mmio::delay_ms(ms),
            }}
        }}
    }}
    Ok(())
}}

{body}
/// Full PS7 bring-up. UART is not usable until this returns Ok.
pub unsafe fn bringup() -> Result<(), u32> {{
    mio()?;
    pll()?;
    clocks()?;
    ddr()?;
    peripherals()?;
    Ok(())
}}

pub unsafe fn ddr_smoke(addr: usize) -> bool {{
    let p = addr as *mut u32;
    let pattern = 0xA5A5_5A5Au32;
    core::ptr::write_volatile(p, pattern);
    core::ptr::write_volatile(p.add(1), !pattern);
    core::ptr::read_volatile(p) == pattern && core::ptr::read_volatile(p.add(1)) == !pattern
}}
"""
    OUT.write_text(out, encoding="utf-8")
    print("wrote", OUT)


if __name__ == "__main__":
    main()
