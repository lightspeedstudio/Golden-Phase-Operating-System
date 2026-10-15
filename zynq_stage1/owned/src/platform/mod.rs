// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! Owned PZ7020 PS7 / UART / WDT / boot-mode (System 3).

pub mod boot_mode;
pub mod entry;
pub mod mmio;
pub mod ops_data;
pub mod uart;
pub mod wdt;

use mmio::Io;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Write { addr: u32, value: u32 },
    MaskWrite { addr: u32, mask: u32, value: u32 },
    Clear { addr: u32 },
    Poll { addr: u32, mask: u32 },
    DelayMs { ms: u32 },
}

pub fn run_ops<I: Io>(io: &mut I, ops: &[Op]) -> Result<(), u32> {
    for op in ops {
        match *op {
            Op::Write { addr, value } => io.write(addr, value),
            Op::Clear { addr } => io.write(addr, 0),
            Op::MaskWrite { addr, mask, value } => {
                let cur = io.read(addr);
                io.write(addr, (value & mask) | (cur & !mask));
            }
            Op::Poll { addr, mask } => io.poll(addr, mask)?,
            Op::DelayMs { ms } => io.delay_ms(ms),
        }
    }
    Ok(())
}

/// Owned order: MIO → PLL → CLOCK → UART → DDR → remaining peripherals.
pub fn sys03_ops() -> impl Iterator<Item = Op> {
    ops_data::MIO_OPS
        .iter()
        .chain(ops_data::PLL_OPS)
        .chain(ops_data::CLOCK_OPS)
        .chain(ops_data::UART_OPS)
        .chain(ops_data::DDR_OPS)
        .chain(ops_data::PERIPH_REST_OPS)
        .copied()
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use crate::sha256::sha256;
    use std::fs;
    use std::path::PathBuf;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn hex32(h: [u8; 32]) -> String {
        h.iter().map(|b| format!("{:02x}", b)).collect()
    }

    fn parse_jsonl_line(l: &str) -> Op {
        fn field<'a>(l: &'a str, k: &str) -> Option<&'a str> {
            let pat = format!("\"{k}\": ");
            let rest = l.split(&pat).nth(1)?;
            if rest.starts_with('"') {
                Some(rest.trim_start_matches('"').split('"').next().unwrap())
            } else {
                Some(rest.split(&[',', '}'][..]).next().unwrap().trim())
            }
        }
        fn hex(s: &str) -> u32 {
            u32::from_str_radix(s.trim_start_matches("0x"), 16).unwrap()
        }
        match field(l, "op").unwrap() {
            "WRITE" => Op::Write {
                addr: hex(field(l, "addr").unwrap()),
                value: hex(field(l, "value").unwrap()),
            },
            "CLEAR" => Op::Clear {
                addr: hex(field(l, "addr").unwrap()),
            },
            "MASKWRITE" => Op::MaskWrite {
                addr: hex(field(l, "addr").unwrap()),
                mask: hex(field(l, "mask").unwrap()),
                value: hex(field(l, "value").unwrap()),
            },
            "MASKPOLL" => Op::Poll {
                addr: hex(field(l, "addr").unwrap()),
                mask: hex(field(l, "mask").unwrap()),
            },
            "MASKDELAY" => Op::DelayMs {
                ms: field(l, "delay_ms").unwrap().parse().unwrap(),
            },
            other => panic!("{other}"),
        }
    }

    fn load_ops(rel: &str) -> Vec<Op> {
        fs::read_to_string(repo_root().join(rel))
            .unwrap()
            .lines()
            .filter(|l| !l.is_empty())
            .map(parse_jsonl_line)
            .collect()
    }

    #[test]
    fn working_trace_locked_length_and_order() {
        let w = load_ops("docs/boot/SYS03_WORKING_PS7_TRACE.jsonl");
        assert_eq!(w.len(), 209);
        assert_eq!(
            w[0],
            Op::Write {
                addr: 0xF800_0008,
                value: 0xDF0D
            }
        );
        let b40 = w
            .iter()
            .position(|o| matches!(o, Op::MaskWrite { addr: 0xF8000B40, .. }))
            .unwrap();
        let ddr = w
            .iter()
            .position(|o| matches!(o, Op::MaskWrite { addr: 0xF8006000, .. }))
            .unwrap();
        let b74 = w
            .iter()
            .position(|o| matches!(o, Op::Poll { addr: 0xF8000B74, .. }))
            .unwrap();
        assert!(b40 < ddr);
        assert!(ddr < b74);
        let uart = w
            .iter()
            .position(|o| matches!(o, Op::MaskWrite { addr: 0xE0000018, .. }))
            .unwrap();
        assert!(ddr < uart);
    }

    #[test]
    fn sys03_trace_matches_expected_uart_early() {
        let exp = load_ops("docs/boot/SYS03_OWNED_PS7_TRACE.expected.jsonl");
        let got: Vec<Op> = sys03_ops().collect();
        assert_eq!(got, exp);
        let uart = got
            .iter()
            .position(|o| matches!(o, Op::MaskWrite { addr: 0xE0000018, .. }))
            .unwrap();
        let ddr = got
            .iter()
            .position(|o| matches!(o, Op::MaskWrite { addr: 0xF8006000, .. }))
            .unwrap();
        assert!(uart < ddr);
    }

    #[test]
    fn system1_hashes_preserved() {
        let main = fs::read(repo_root().join("zynq_stage1/src/main.rs")).unwrap();
        assert_eq!(main.len(), 9580);
        assert_eq!(
            hex32(sha256(&main)),
            "6637678d0e01b007caf3b5e150ed37e48ed5af6f55cc009a3b16e0d5c4ebd67d"
        );
        let d = fs::read(repo_root().join("src/board/zynq7000/ps7_init_data.rs")).unwrap();
        assert_eq!(
            hex32(sha256(&d)),
            "37b5710184351c7aaa13c5fdf485702a83455ad30a07d4203292a323fee3492b"
        );
    }

    #[test]
    fn required_uart_strings_present() {
        let e = include_str!("entry.rs");
        for s in [
            "GP-S1 BEGIN",
            "UART EARLY PASS",
            "PS7 CLOCK PASS",
            "DDR BEGIN",
            "DDR PASS",
            "WDT STOP PASS",
            "BOOTMODE SD PASS",
            "SYS03 HOLD",
        ] {
            assert!(e.contains(s), "{s}");
        }
    }

    #[test]
    fn host_bringup_reaches_hold() {
        use super::entry::sys03_bringup;
        use super::mmio::HostIo;
        let mut io = HostIo::new();
        io.mem.insert(0xF800_025C, 0x5);
        assert_eq!(sys03_bringup(&mut io), Ok(()));
    }
}
