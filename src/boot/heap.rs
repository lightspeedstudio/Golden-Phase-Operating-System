//! Global heap for bare-metal builds.

use linked_list_allocator::LockedHeap;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

const HEAP_BYTES: usize = 512 * 1024;
static mut HEAP_MEM: [u8; HEAP_BYTES] = [0; HEAP_BYTES];

pub fn init() {
    let (start, size) = heap_region();
    unsafe {
        ALLOCATOR.lock().init(start, size);
    }
}

fn heap_region() -> (*mut u8, usize) {
    #[cfg(all(feature = "baremetal", target_arch = "riscv64"))]
    {
        #[allow(static_mut_refs)]
        unsafe {
            let start = HEAP_MEM.as_mut_ptr();
            (start, HEAP_BYTES)
        }
    }
    #[cfg(not(all(feature = "baremetal", target_arch = "riscv64")))]
    {
        extern "C" {
            static __heap_start: u8;
            static __heap_end: u8;
        }
        let start = core::ptr::addr_of!(__heap_start) as usize;
        let end = core::ptr::addr_of!(__heap_end) as usize;
        (start as *mut u8, end.saturating_sub(start))
    }
}
