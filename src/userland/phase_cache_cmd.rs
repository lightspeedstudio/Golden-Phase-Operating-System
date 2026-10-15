//! `phasecache` — print `kernel.phase_cache`'s live hit/miss/trust-rejection
//! counters and current size. Read-only; exists so the cache's real
//! behavior (not just its presence) is directly observable from the
//! console/serial, on both hosted and bare-metal builds.

use crate::kernel::Kernel;
use crate::platform::String;

pub fn render(kernel: &Kernel) -> String {
    #[cfg(any(feature = "std", feature = "baremetal"))]
    {
        let stats = kernel.phase_cache.stats();
        format!(
            "phasecache\n\
             entries: {}\n\
             hits: {}\n\
             misses: {}\n\
             trust_rejections: {}",
            kernel.phase_cache.len(),
            stats.hits,
            stats.misses,
            stats.trust_rejections,
        )
    }
    #[cfg(not(any(feature = "std", feature = "baremetal")))]
    {
        let _ = kernel;
        "phasecache unavailable (statechain feature required)".into()
    }
}
