//! `pmm` / `pmm selftest` — observe `kernel.physical_memory`'s live counters
//! and, on request, run a real alloc/sentinel-write/verify/free/realloc round
//! trip. Exists so subsystem 1's behavior is directly observable from the
//! console/serial on hosted and bare-metal builds, matching the `phasecache`
//! command's precedent.

use crate::kernel::Kernel;
use crate::platform::String;

pub fn render(kernel: &Kernel) -> String {
    #[cfg(all(feature = "pmm", any(feature = "std", feature = "baremetal")))]
    {
        let stats = kernel.physical_memory.stats();
        format!(
            "pmm\n\
             frames: {}\n\
             max_order: {}\n\
             allocations: {}\n\
             frees: {}\n\
             double_free_rejections: {}\n\
             exhaustions: {}\n\
             quarantines: {}",
            kernel.physical_memory.frame_count(),
            kernel.physical_memory.max_order(),
            stats.allocations,
            stats.frees,
            stats.double_free_rejections,
            stats.exhaustions,
            stats.quarantines,
        )
    }
    #[cfg(not(all(feature = "pmm", any(feature = "std", feature = "baremetal"))))]
    {
        let _ = kernel;
        "pmm unavailable (pmm feature not enabled)".into()
    }
}

#[cfg(all(feature = "pmm", any(feature = "std", feature = "baremetal")))]
pub fn selftest(kernel: &mut Kernel) -> String {
    use crate::platform::Vec;
    const COUNT: u32 = 8;

    let mut addrs: Vec<usize> = Vec::new();
    for _ in 0..COUNT {
        match kernel.physical_memory.alloc(1) {
            Some(a) => addrs.push(a),
            None => {
                for a in &addrs {
                    let _ = kernel.physical_memory.free(*a);
                }
                return format!(
                    "pmm selftest: FAIL (exhausted after {} of {COUNT} allocations)",
                    addrs.len()
                );
            }
        }
    }

    let write_ok = sentinel_round_trip(&addrs, 0xA5A5_A5A5);

    for &addr in &addrs {
        if kernel.physical_memory.free(addr).is_err() {
            return "pmm selftest: FAIL (free rejected on first pass)".into();
        }
    }

    let mut realloc_addrs: Vec<usize> = Vec::new();
    for _ in 0..COUNT {
        match kernel.physical_memory.alloc(1) {
            Some(a) => realloc_addrs.push(a),
            None => {
                for a in &realloc_addrs {
                    let _ = kernel.physical_memory.free(*a);
                }
                return "pmm selftest: FAIL (reallocation exhausted)".into();
            }
        }
    }

    let reverify_ok = sentinel_round_trip(&realloc_addrs, 0x5A5A_5A5A);

    for &addr in &realloc_addrs {
        let _ = kernel.physical_memory.free(addr);
    }

    let sentinel_note = if cfg!(target_arch = "arm") {
        "real sentinel read/write on returned physical addresses"
    } else {
        "sentinel read/write not applicable on hosted build (bookkeeping only)"
    };

    if write_ok && reverify_ok {
        format!(
            "pmm selftest: PASS ({COUNT} frames alloc -> verify -> free -> realloc -> verify, {sentinel_note})"
        )
    } else {
        "pmm selftest: FAIL (sentinel mismatch after alloc/free/realloc)".into()
    }
}

/// Writes a distinct pattern into every address, reads every address back,
/// and reports whether all of them matched. On bare-metal ARM (MMU off,
/// identity-mapped, per `docs/PZ7020_ARCHITECTURE.md`) these are real
/// physical-memory volatile accesses; on hosted builds the addresses are a
/// synthetic simulation range (see `Kernel::new_physical_memory_manager`), so
/// dereferencing them would be undefined behavior — the hosted build skips
/// the actual memory access and only proves the allocator returned distinct,
/// stable addresses across the alloc/free/realloc cycle.
#[cfg(all(feature = "pmm", any(feature = "std", feature = "baremetal")))]
fn sentinel_round_trip(addrs: &[usize], salt: u32) -> bool {
    #[cfg(target_arch = "arm")]
    {
        for (i, &addr) in addrs.iter().enumerate() {
            let pattern = (i as u32) ^ salt;
            unsafe {
                core::ptr::write_volatile(addr as *mut u32, pattern);
            }
        }
        let mut ok = true;
        for (i, &addr) in addrs.iter().enumerate() {
            let expected = (i as u32) ^ salt;
            let observed = unsafe { core::ptr::read_volatile(addr as *const u32) };
            if observed != expected {
                ok = false;
            }
        }
        ok
    }
    #[cfg(not(target_arch = "arm"))]
    {
        let _ = salt;
        let mut unique = crate::platform::Vec::new();
        for &addr in addrs {
            if unique.contains(&addr) {
                return false;
            }
            unique.push(addr);
        }
        true
    }
}
