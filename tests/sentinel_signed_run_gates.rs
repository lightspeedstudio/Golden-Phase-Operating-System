//! Public-API checks for PhaseSentinel-gated `app run`.
//! Gates S0–S12 that need crate-private ticket minting live in
//! `kernel::phasestack::sentinel_gates`.
#![cfg(all(feature = "phasestack", feature = "phasesentinel"))]

use kairos::kernel::phasestack::install;
use kairos::kernel::phasestack::types::PhaseStackError;
use kairos::kernel::Kernel;

#[test]
fn s0_shell_cannot_construct_ticket() {
    // VerifiedLaunchTicket::from_installation is pub(super) in phasestack.
    // This integration crate can only pass an AppId into install::launch.
    let mut kernel = Kernel::new();
    assert_eq!(
        install::launch(&mut kernel, [0u8; 32], 8),
        Err(PhaseStackError::RegistryMissing)
    );
}
