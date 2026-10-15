//! Trap entry for machine-mode faults — prints diagnostics over UART.

#[cfg(all(feature = "baremetal", target_arch = "riscv64"))]
use crate::boot::arch::riscv64::uart;
use core::sync::atomic::{AtomicU32, Ordering};

static SYSCALL_TRAPS: AtomicU32 = AtomicU32::new(0);

#[repr(C)]
pub struct TrapFrame {
    pub sepc: usize,
    pub a0: usize,
    pub a7: usize,
}

pub fn init() {}

/// Reserved for a follow-up once UART shell smoke is stable.
#[allow(dead_code)]
pub fn enable_timer_irq() {
    #[cfg(all(feature = "baremetal", target_arch = "riscv64"))]
    unsafe {
        core::arch::asm!("csrsi mstatus, 8", "csrsi mie, 7", options(nostack, nomem));
    }
}

pub fn syscall_trap_count() -> u32 {
    SYSCALL_TRAPS.load(Ordering::Relaxed)
}

pub fn handle_ecall(frame: &mut TrapFrame) {
    SYSCALL_TRAPS.fetch_add(1, Ordering::Relaxed);
    frame.a0 = 0;
    frame.sepc = frame.sepc.saturating_add(4);
}

#[cfg(all(feature = "baremetal", target_arch = "riscv64"))]
#[no_mangle]
extern "C" fn trap_report(mcause: usize, mepc: usize, mtval: usize) -> ! {
    uart::write_str("trap mcause=0x");
    uart::write_hex(mcause);
    uart::write_str(" mepc=0x");
    uart::write_hex(mepc);
    uart::write_str(" mtval=0x");
    uart::write_hex(mtval);
    uart::write_str("\n");
    loop {
        unsafe {
            core::arch::asm!("wfi", options(nomem, nostack));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ecall_trap_increments_counter() {
        let before = syscall_trap_count();
        let mut frame = TrapFrame {
            sepc: 0x8000_0000,
            a0: 0,
            a7: 1,
        };
        handle_ecall(&mut frame);
        assert_eq!(syscall_trap_count(), before + 1);
        assert_eq!(frame.sepc, 0x8000_0004);
    }
}
