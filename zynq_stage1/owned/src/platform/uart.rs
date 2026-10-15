// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! UART0 after clocks. Register values from working `peripherals()`.

use super::mmio::Io;
use super::ops_data::UART_OPS;
use super::run_ops;

const UART0: u32 = 0xE000_0000;
const SR: u32 = UART0 + 0x2C;
const FIFO: u32 = UART0 + 0x30;
const SR_TXEMPTY: u32 = 1 << 3;
const SR_TACTIVE: u32 = 1 << 11;

pub fn init<I: Io>(io: &mut I) -> Result<(), u32> {
    run_ops(io, UART_OPS)
}

pub fn write_str<I: Io>(io: &mut I, s: &str) {
    for b in s.bytes() {
        if b == b'\n' {
            putc(io, b'\r');
        }
        putc(io, b);
    }
}

/// Wait until TX FIFO is empty and the shift register has finished the last byte.
/// `write_str` only waits TXEMPTY before each store; the last byte can still be shifting
/// when `handoff::jump` re-inits UART0.
pub fn wait_tx_empty<I: Io>(io: &mut I) {
    wait_sr(io, SR_TXEMPTY, true);
    wait_sr(io, SR_TACTIVE, false);
}

fn wait_sr<I: Io>(io: &mut I, mask: u32, set: bool) {
    let mut spins = 0u32;
    loop {
        let bit = (io.read(SR) & mask) != 0;
        if bit == set {
            return;
        }
        if spins == super::mmio::POLL_SPINS {
            return;
        }
        spins += 1;
    }
}

fn putc<I: Io>(io: &mut I, c: u8) {
    // 1 byte at 115200 8N1 ≈ 87 µs (10/115200 s). Bound with working 1 ms MASKDELAY budget.
    wait_sr(io, SR_TXEMPTY, true);
    io.write(FIFO, c as u32);
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use crate::platform::mmio::HostIo;

    #[test]
    fn wait_tx_empty_returns_when_fifo_idle() {
        let mut io = HostIo::new();
        wait_tx_empty(&mut io);
        write_str(&mut io, "JUMP 00100060\n");
        wait_tx_empty(&mut io);
        let fifo = io.mem.get(&FIFO).copied().unwrap();
        assert_eq!(fifo, b'\n' as u32);
    }
}
