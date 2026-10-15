//! `extmap` — observe `kernel.extmap`'s live block-pool / extent counters
//! (subsystem 6).

use crate::kernel::Kernel;
use crate::platform::String;

pub fn render(kernel: &Kernel) -> String {
    #[cfg(all(feature = "extmap", any(feature = "std", feature = "baremetal")))]
    {
        let stats = kernel.extmap.stats();
        format!(
            "extmap\n\
             blocks_total: {}\n\
             blocks_free: {}\n\
             blocks_reserved: {}\n\
             blocks_writing: {}\n\
             blocks_live: {}\n\
             blocks_stale: {}\n\
             blocks_damaged: {}\n\
             allocations: {}\n\
             extent_writes: {}\n\
             writes: {}\n\
             reads: {}\n\
             deletes: {}\n\
             frees: {}\n\
             splits: {}\n\
             coalesces: {}\n\
             resizes: {}\n\
             dedup_hits: {}\n\
             out_of_space_rejections: {}",
            stats.blocks_total,
            stats.blocks_free,
            stats.blocks_reserved,
            stats.blocks_writing,
            stats.blocks_live,
            stats.blocks_stale,
            stats.blocks_damaged,
            stats.allocations,
            stats.extent_writes,
            stats.writes,
            stats.reads,
            stats.deletes,
            stats.frees,
            stats.splits,
            stats.coalesces,
            stats.resizes,
            stats.dedup_hits,
            stats.out_of_space_rejections,
        )
    }
    #[cfg(not(all(feature = "extmap", any(feature = "std", feature = "baremetal"))))]
    {
        let _ = kernel;
        "extmap unavailable (extmap feature not enabled)".into()
    }
}
