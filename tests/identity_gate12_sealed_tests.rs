//! Gate 12: `identity` is default-OFF and must not rewrite sealed-boot
//! `BootSessionRoot` / object-kind tables when the feature is absent.
#![cfg(not(feature = "identity"))]

use kairos::kernel::Kernel;
use kairos::kernel::state_proof::ObjectKind;
use kairos::kernel::state_root::SubsystemKind;

#[test]
fn default_features_do_not_enable_identity() {
    let cargo = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));
    let default_line = cargo
        .lines()
        .find(|l| l.starts_with("default ="))
        .expect("Cargo.toml default features line");
    assert!(
        !default_line.contains("identity"),
        "identity leaked into default features: {default_line}"
    );
}

#[test]
fn sealed_boot_session_root_stays_uninitialized_without_identity() {
    let kernel = Kernel::new();
    assert_eq!(kernel.authority.boot_session_root, [0u8; 32]);
}

#[test]
fn object_kind_and_subsystem_kind_tables_were_not_extended_for_identity() {
    assert_eq!(ObjectKind::Craft.as_u8(), 8);
    #[cfg(not(feature = "phasestack"))]
    assert!(ObjectKind::from_u8(9).is_none());
    #[cfg(feature = "phasestack")]
    assert_eq!(ObjectKind::from_u8(9), Some(ObjectKind::App));
    assert_eq!(SubsystemKind::Craft.as_u8(), 6);
    assert!(SubsystemKind::from_u8(7).is_none());
}
