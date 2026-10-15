//! Hosted SDK helpers. Never part of the sealed kernel image path.

#[cfg(all(feature = "std", feature = "phasestack-sign-dev"))]
pub mod phasestack;
