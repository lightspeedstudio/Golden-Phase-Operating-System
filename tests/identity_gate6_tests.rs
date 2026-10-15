//! Gate 6 evidence: identity snapshots persist through a real `std::fs`
//! file (the hosted equivalent of SD FAT32 `IDSTORE.BIN`) and reload into
//! a fresh `Identity` / `Kernel`.
#![cfg(feature = "identity")]

use kairos::kernel::identity::store::StdFsStore;
use kairos::kernel::identity::{PrincipalKind, RootCredentialState};
use kairos::runtime::Runtime;

fn temp_store() -> StdFsStore {
    let path = std::env::temp_dir().join(format!(
        "kairos-idstore-{}-{}.bin",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _ = std::fs::remove_file(&path);
    StdFsStore { path }
}

#[test]
fn persist_and_restore_preserves_a_provisioned_root_password() {
    let mut rt = Runtime::with_input([
        "correct horse battery staple one",
        "correct horse battery staple one",
    ]);
    rt.boot().unwrap();
    rt.run_command("passwd").unwrap();
    assert_eq!(
        rt.hosted.kernel.identity.root_credential().state(),
        RootCredentialState::Active
    );

    let mut store = temp_store();
    rt.hosted.kernel.identity_persist(&mut store).unwrap();
    let path = store.path.clone();

    let mut rt2 = Runtime::new();
    rt2.boot().unwrap();
    assert_eq!(
        rt2.hosted.kernel.identity.root_credential().state(),
        RootCredentialState::Unprovisioned
    );
    rt2.hosted.kernel.identity_restore(&mut store).unwrap();
    assert_eq!(
        rt2.hosted.kernel.identity.root_credential().state(),
        RootCredentialState::Active
    );
    assert_eq!(
        rt2.hosted.kernel.identity.ledger().sequence(),
        rt.hosted.kernel.identity.ledger().sequence()
    );
    assert_eq!(
        rt2.hosted.kernel.identity.authority_state_root(),
        rt.hosted.kernel.identity.authority_state_root()
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn persist_and_restore_preserves_principals_created_through_the_kernel() {
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    let id = rt
        .hosted
        .kernel
        .identity_create_principal(PrincipalKind::Operator)
        .unwrap();

    let mut store = temp_store();
    rt.hosted.kernel.identity_persist(&mut store).unwrap();
    let path = store.path.clone();

    let mut rt2 = Runtime::new();
    rt2.boot().unwrap();
    rt2.hosted.kernel.identity_restore(&mut store).unwrap();
    assert_eq!(
        rt2.hosted.kernel.identity.principals().get(id).unwrap().kind,
        PrincipalKind::Operator
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn missing_store_file_is_not_found_not_a_panic() {
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    let mut store = temp_store();
    assert_eq!(
        rt.hosted.kernel.identity_restore(&mut store),
        Err(kairos::kernel::identity::StoreError::NotFound)
    );
}
