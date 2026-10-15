//! Golden Phase OS — Zynq A9 memory-system acceptance (bare metal).
//!
//! Boot: FSBL → V2 system.bit → this ELF @ DDR 0x00100000
//! Proves phase-tagged MM from `kernel::memory`, not vendor DRAM soak.

#![no_std]
#![no_main]

extern crate alloc;

use kairos::arch::armv7;
use kairos::board::zynq7000::uart;
use kairos::core::gphase_core::{compose, PhaseState};
use kairos::kernel::memory::Memory;
use kairos::kernel::phase_consensus::PhaseConsensus;

// Force Cortex-A9 startup + heap into the link.
#[allow(unused_imports)]
use armv7 as _;

fn fail(msg: &str) -> ! {
    uart::write_str("FAIL: ");
    uart::write_str(msg);
    uart::write_str("\n");
    loop {
        unsafe {
            core::arch::asm!("wfi", options(nomem, nostack));
        }
    }
}

fn expect_tag(mem: &Memory, page: u32, want: PhaseState, ctx: &str) {
    match mem.get_tag(page) {
        Ok(t) if t == want => {}
        Ok(t) => {
            let _ = t;
            fail(ctx);
        }
        Err(_) => fail(ctx),
    }
}

#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    kairos::arch::armv7::heap::init();

    uart::write_str("\nKAIROS GOLDEN PHASE OS\n");
    uart::write_str("AARCH32 STARLITE BOOT PASS\n");
    uart::write_str("GP MEMORY SYSTEM TEST\n");

    let mut mem = Memory::new();

    let p0 = mem.alloc_page();
    expect_tag(&mem, p0, PhaseState::Phi0, "alloc not Phi0");
    uart::write_str("ALLOC Phi0 PASS\n");

    if mem.map_user(p0).is_err() {
        fail("map_user");
    }
    expect_tag(&mem, p0, PhaseState::Phi1, "map_user not Phi1");
    uart::write_str("MAP USER PASS\n");

    if mem.escalate(p0).is_err() {
        fail("escalate");
    }
    expect_tag(&mem, p0, PhaseState::Phi2, "escalate not Phi2");
    uart::write_str("ESCALATE PASS\n");

    if mem.lock_page(p0).is_err() {
        fail("lock");
    }
    expect_tag(&mem, p0, PhaseState::Phi3, "lock not Phi3");
    uart::write_str("LOCK PASS\n");

    if mem.unlock_page(p0).is_err() {
        fail("unlock");
    }
    expect_tag(&mem, p0, PhaseState::Phi2, "unlock not Phi2");
    uart::write_str("UNLOCK PASS\n");

    if mem.unmap_kernel(p0).is_err() {
        fail("unmap_kernel");
    }
    expect_tag(&mem, p0, PhaseState::Phi0, "unmap not Phi0");
    if mem.free_page(p0).is_err() {
        fail("free");
    }
    uart::write_str("UNMAP FREE PASS\n");

    /* Illegal: free while mapped must fail */
    let p1 = mem.alloc_page();
    let _ = mem.map_user(p1);
    if mem.free_page(p1).is_ok() {
        fail("illegal free should err");
    }
    uart::write_str("ILLEGAL FREE REJECT PASS\n");

    let p2 = mem.alloc_page();
    let _ = mem.map_user(p2);
    let p3 = mem.alloc_page();
    let _ = mem.map_kernel(p3);
    match mem.arbitrate(p2, p3, &PhaseConsensus::emulated()) {
        Ok(r) => {
            let expect = compose(PhaseState::Phi1, PhaseState::Phi2);
            if r != expect {
                fail("compose mismatch");
            }
            uart::write_str("ARBITRATE COMPOSE PASS\n");
        }
        Err(_) => fail("arbitrate"),
    }

    uart::write_str("GP MEMORY SYSTEM PASS\n");
    uart::write_str("HEARTBEAT\n");
    loop {
        unsafe {
            core::arch::asm!("wfi", options(nomem, nostack));
        }
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    uart::write_str("panic\n");
    loop {
        unsafe {
            core::arch::asm!("wfi", options(nomem, nostack));
        }
    }
}
