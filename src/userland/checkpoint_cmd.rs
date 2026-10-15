//! `checkpoint` — seal a kernel-root integrity MAC (HMAC-SHA256).

use crate::kernel::Kernel;
use crate::platform::String;

// `StatechainLedger::seal_checkpoint` has no `std`-only dependency (pure
// HMAC-SHA256 integrity MAC over integers and hashes) — it is real on bare
// metal too, gated the same as `kernel.statechain` itself.
#[cfg(any(feature = "std", feature = "baremetal"))]
use crate::kernel::statechain_checkpoint::CheckpointError;
#[cfg(any(feature = "std", feature = "baremetal"))]
use crate::userland::terminal_fmt::short_hex;

pub fn render(kernel: &mut Kernel) -> Result<String, String> {
    #[cfg(any(feature = "std", feature = "baremetal"))]
    {
        match {
            #[cfg(feature = "phasestack")]
            {
                let app_root = kernel.phasestack.registry_root();
                kernel.statechain.seal_checkpoint_v2(app_root)
            }
            #[cfg(not(feature = "phasestack"))]
            {
                kernel.statechain.seal_checkpoint()
            }
        } {
            Ok(cp) => {
                #[cfg(feature = "phasestack")]
                {
                    Ok(format!(
                        "checkpoint sealed\n\
                         class: INTEGRITY_ONLY\n\
                         mac: HMAC-SHA256\n\
                         key: public-derivable\n\
                         sequence: {}\n\
                         tick: {}\n\
                         kernel_root: {}…\n\
                         app_registry_root: {}…\n\
                         hmac: {}…",
                        cp.sequence,
                        cp.tick,
                        short_hex(&cp.kernel_root),
                        short_hex(&cp.app_registry_root),
                        short_hex(&cp.signature)
                    ))
                }
                #[cfg(not(feature = "phasestack"))]
                {
                    Ok(format!(
                        "checkpoint sealed\n\
                         class: INTEGRITY_ONLY\n\
                         mac: HMAC-SHA256\n\
                         key: public-derivable\n\
                         sequence: {}\n\
                         tick: {}\n\
                         kernel_root: {}…\n\
                         hmac: {}…",
                        cp.sequence,
                        cp.tick,
                        short_hex(&cp.kernel_root),
                        short_hex(&cp.signature)
                    ))
                }
            }
            Err(CheckpointError::Empty) => Err("checkpoint failed: empty kernel root".into()),
            Err(CheckpointError::BadSignature) => Err("checkpoint failed: bad integrity mac".into()),
        }
    }
    #[cfg(not(any(feature = "std", feature = "baremetal")))]
    {
        let _ = kernel;
        Err("checkpoint unavailable (statechain feature required)".into())
    }
}
