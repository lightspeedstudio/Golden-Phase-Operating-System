//! `audit` — run StatechainWatchdog and print report.

use crate::kernel::Kernel;
use crate::platform::String;

#[cfg(any(feature = "std", feature = "baremetal"))]
use crate::kernel::statechain_watchdog::StatechainWatchdog;
#[cfg(any(feature = "std", feature = "baremetal"))]
use crate::userland::terminal_fmt::short_hex;

pub fn render(kernel: &Kernel) -> String {
    #[cfg(any(feature = "std", feature = "baremetal"))]
    {
        let dog = StatechainWatchdog::new();
        let report = dog.audit(&kernel.statechain);
        let verdict = if report.tamper_detected {
            "FAIL"
        } else {
            "PASS"
        };
        format!(
            "audit {verdict}\n\
             equation: {}\n\
             chains: {}\n\
             kernel_root: {}\n\
             checkpoints: {}\n\
             stored: {}…\n\
             recomputed: {}…\n\
             {}",
            pass_fail(report.equation_ok),
            pass_fail(report.chains_ok),
            pass_fail(report.kernel_root_ok),
            pass_fail(report.signed_checkpoints_ok),
            short_hex(&report.stored_root),
            short_hex(&report.recomputed_root),
            report.message
        )
    }
    #[cfg(not(any(feature = "std", feature = "baremetal")))]
    {
        let _ = kernel;
        "audit unavailable (no statechain watchdog on this build)".into()
    }
}

#[cfg(any(feature = "std", feature = "baremetal"))]
fn pass_fail(ok: bool) -> &'static str {
    if ok {
        "PASS"
    } else {
        "FAIL"
    }
}
