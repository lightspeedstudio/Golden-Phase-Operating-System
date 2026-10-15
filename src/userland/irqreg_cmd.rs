//! `irqreg` — observe the live IRQ and device registry (subsystem 3).
//!
//! Unlike `pmm`/`captable`, the live registry is a `board::zynq7000::gic`
//! singleton, not a `Kernel` field — IRQ dispatch is inherently board/
//! hardware-scoped (there is no GIC on the hosted build). `kernel` is taken
//! only for signature parity with the other status commands and to keep the
//! `emit()`-based dispatch plumbing in `terminal_cmd.rs` uniform.

use crate::kernel::Kernel;
use crate::platform::String;

pub fn render(kernel: &Kernel) -> String {
    let _ = kernel;
    #[cfg(all(target_arch = "arm", feature = "irqreg"))]
    {
        use crate::board::zynq7000::gic;
        let stats = gic::registry_stats();
        format!(
            "irqreg\n\
             g1p_irq_61:   count={} storm_masked={}\n\
             gem0_rx_54:   count={} storm_masked={}\n\
             usb0_53:      count={} storm_masked={}\n\
             priv_timer_29: count={} storm_masked={}\n\
             ring_dropped: {}\n\
             ring_pending: {}\n\
             dispatch_rejected: {}",
            gic::irq_count(),
            gic::is_storm_masked(61),
            gic::gem_rx_irq_count(),
            gic::is_storm_masked(54),
            {
                #[cfg(feature = "usb")]
                {
                    gic::usb0_irq_count()
                }
                #[cfg(not(feature = "usb"))]
                {
                    0
                }
            },
            gic::is_storm_masked(53),
            gic::tick_count(),
            gic::is_storm_masked(29),
            stats.ring_dropped,
            stats.ring_pending,
            stats.dispatch_rejected,
        )
    }
    #[cfg(not(all(target_arch = "arm", feature = "irqreg")))]
    {
        "irqreg unavailable (irqreg feature not enabled, or not a PZ7020 build)".into()
    }
}
