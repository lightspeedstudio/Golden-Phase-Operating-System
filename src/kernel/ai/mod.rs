//! KairOS AI control layer — jobs, protocol, tools, PhaseLink inbound.

pub mod error;
pub mod protocol;

#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod permissions;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod tools;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod trace;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod types;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod control;

#[cfg(feature = "std")]
pub mod bridge;
#[cfg(feature = "std")]
pub mod client;
#[cfg(feature = "std")]
pub mod mock;

#[cfg(all(target_arch = "arm", feature = "ssh-bm"))]
pub mod client_bm;

#[cfg(all(target_arch = "arm", feature = "bm-net"))]
pub mod inbound_bm;

#[cfg(feature = "std")]
pub use bridge::{connect, disconnect, poll_completions, status};
pub use error::AiError;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub use types::{AiBridgeController, AiJob, AiJobStatus};
