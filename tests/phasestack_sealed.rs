//! Sealed-off: `phasestack` must not be in default features.
#![cfg(not(feature = "phasestack"))]

use kairos::kernel::state_proof::ObjectKind;
use kairos::userland::parser::{parse, Command};

#[test]
fn sealed_object_kind_has_no_app() {
    assert_eq!(ObjectKind::from_u8(9), None);
}

#[test]
fn app_command_parses_but_feature_is_off() {
    match parse("app list") {
        Command::App { args } => assert_eq!(args, vec!["list".to_string()]),
        other => panic!("expected App, got {other:?}"),
    }
}
