#![no_std]
#![no_main]

mod gphase;
mod memory;
mod uart;

use gphase::{compose, PhaseState};
use memory::Memory;

core::arch::global_asm!(include_str!("startup.S"));

fn fail(msg: &str) -> ! {
    uart::write_str("FAIL: ");
    uart::write_str(msg);
    uart::write_str("\n");
    halt();
}

fn halt() -> ! {
    loop {
        unsafe {
            core::arch::asm!("wfi", options(nomem, nostack));
        }
    }
}

fn expect(mem: &Memory, page: u32, want: PhaseState, ctx: &str) {
    match mem.get_tag(page) {
        Ok(t) if t == want => {}
        _ => fail(ctx),
    }
}

#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    uart::write_str("\nKAIROS GOLDEN PHASE OS\n");
    uart::write_str("AARCH32 STARLITE BOOT PASS\n");
    uart::write_str("GP MEMORY SYSTEM TEST\n");

    let mut mem = Memory::new();

    let p0 = mem.alloc_page();
    expect(&mem, p0, PhaseState::Phi0, "alloc not Phi0");
    uart::write_str("ALLOC Phi0 PASS\n");

    if mem.map_user(p0).is_err() {
        fail("map_user");
    }
    expect(&mem, p0, PhaseState::Phi1, "map_user");
    uart::write_str("MAP USER PASS\n");

    if mem.escalate(p0).is_err() {
        fail("escalate");
    }
    expect(&mem, p0, PhaseState::Phi2, "escalate");
    uart::write_str("ESCALATE PASS\n");

    if mem.lock_page(p0).is_err() {
        fail("lock");
    }
    expect(&mem, p0, PhaseState::Phi3, "lock");
    uart::write_str("LOCK PASS\n");

    if mem.unlock_page(p0).is_err() {
        fail("unlock");
    }
    expect(&mem, p0, PhaseState::Phi2, "unlock");
    uart::write_str("UNLOCK PASS\n");

    if mem.unmap_kernel(p0).is_err() {
        fail("unmap_kernel");
    }
    expect(&mem, p0, PhaseState::Phi0, "unmap");
    if mem.free_page(p0).is_err() {
        fail("free");
    }
    uart::write_str("UNMAP FREE PASS\n");

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
    match mem.arbitrate(p2, p3) {
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
    halt();
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    uart::write_str("panic\n");
    halt();
}
