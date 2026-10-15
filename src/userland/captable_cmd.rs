//! `captable` — observe `kernel.captable`'s live object/capability counters.
//! `open`/`close` (the actual `FS_OPEN`/`FS_CLOSE` syscalls this table
//! backs) live in `userland::fs_cmd`, matching where every other FS_*
//! command lives.

use crate::kernel::Kernel;
use crate::platform::String;

pub fn render(kernel: &Kernel) -> String {
    #[cfg(all(feature = "captable", any(feature = "std", feature = "baremetal")))]
    {
        let stats = kernel.captable.stats();
        format!(
            "captable\n\
             live_objects: {}\n\
             live_capabilities: {}\n\
             objects_created: {}\n\
             objects_destroyed: {}\n\
             opens: {}\n\
             closes: {}\n\
             delegations: {}\n\
             authorize_ok: {}\n\
             authorize_denied_stale: {}\n\
             authorize_denied_rights: {}\n\
             authorize_denied_subject: {}\n\
             authorize_denied_expiry: {}\n\
             authorize_denied_chain: {}\n\
             generation_retired: {}",
            kernel.captable.object_count(),
            kernel.captable.capability_count(),
            stats.objects_created,
            stats.objects_destroyed,
            stats.opens,
            stats.closes,
            stats.delegations,
            stats.authorize_ok,
            stats.authorize_denied_stale,
            stats.authorize_denied_rights,
            stats.authorize_denied_subject,
            stats.authorize_denied_expiry,
            stats.authorize_denied_chain,
            stats.generation_retired,
        )
    }
    #[cfg(not(all(feature = "captable", any(feature = "std", feature = "baremetal"))))]
    {
        let _ = kernel;
        "captable unavailable (captable feature not enabled)".into()
    }
}
