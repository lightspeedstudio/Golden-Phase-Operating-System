//! PhaseStack verified runtime (`feature = "phasestack"`).
//!
//! Utility images pin only `ROOT_PUBLIC_KEY_PIN` and refuse
//! `RETIRED_RESEARCH_CA_PIN`. Signing lives in hosted `phasestack-sign-dev`.
//!
//! # Gate STATUS
//! - **A** files: `cert.rs`, `canonical.rs`, `types.rs`, PhaseHash domains.
//!   Real: DeveloperCertificateV1, TBS/hash, Ed25519 `sign::verify`, chain
//!   walk to embedded Root public key, generation/mask/cycle/serial/depth
//!   reject. Incomplete: none for A. Tests: unit in `cert.rs`. Evidence:
//!   host `cargo test`. Hardware: none. Commit: not unless asked.
//! - **B** files: `parser.rs`. Real GPA1 + canonical manifests + PackageRoot
//!   inputs + resource Merkle. Tests: traversal negatives.
//! - **C** files: `verifier.rs`, `revocation.rs`. Real Sign(PackageRoot),
//!   unknown/wrong/revoked/expired.
//! - **D** files: private `VerifiedPackage`/`VerifiedInstallation`;
//!   `PhaseAppLoader::load(&VerifiedInstallation)` only.
//! - **E** files: `install.rs`, `native.rs`, `vm_runtime.rs`. Immutable
//!   store + launch re-hash + PhaseVM execute. Package store files go
//!   through `syscall::persist_write_file` (SD/host WAL when
//!   `phasefs-journal` is on; RAM `txlog` only as persist fallback).
//!   Native ARM ELF32 ET_EXEC loads into the isolated user window when
//!   `mmu-app-isolation` is live on Cortex-A9. Host / isolation-off still
//!   returns `NativeIsolationUnavailable`. Raw A32 non-ELF uses that same
//!   window; `mmu_wx_status` / `elf_execute_status` report the split.
//!   Not Linux ABI, not dynamic linking.
//! - **F** files: `hooks.rs`. Single `require_delivery`, bounds.
//! - **G** plugin VM in `vm_runtime::run_plugin` + cap intersection.
//! - **H** shell: `userland/phasestack_cmd.rs`.
//! - **I** `ph_app_registry` + Statechain `ObjectKind::App` + checkpoint V2.
//!   Package blobs can land in the PhaseFS WAL. After WAL recover and
//!   extmap rebuild, `rebuild_from_store` re-verifies recovered
//!   `/phase/packages/<id>/package.gpa` files through `verify_package`
//!   and reconstructs AppRegistry. Directories without a verifiable GPA
//!   are not registered.
//! - **J** `negative.rs` 1–51 (39 raw-A32 status split, 40 host ELF execute
//!   unavailable, 51 native ELF verifies then host launch unavailable);
//!   ARM `kairos_zynq.elf` string scan: no `phasestack-dev`, no
//!   `KAIROS_PHASESTACK_RESEARCH_CA_SEED_V1`, no bypass symbols. Hardware: none.
//! - **K** PZ7020 COM12 2026-08-13. Image
//!   `BOOT_PHASESTACK_BAREMETAL_20260813`. UART
//!   `com12_phasestack_gatek_20260813_152024.txt`. AppRegistryRoot `975cfcd8`
//!   (empty). `--no-verify` rejected on the live shell.

pub mod canonical;
pub mod capability;
pub mod cert;
pub mod hooks;
pub mod install;
pub mod ledger;
pub mod loader;
pub mod native;
pub mod parser;
pub mod registry;
pub mod revocation;
pub mod run;
pub mod supervisor;
pub mod ticket;
pub mod types;
pub mod verifier;
pub mod vm_runtime;
#[cfg(test)]
mod negative;
#[cfg(all(test, feature = "phasesentinel"))]
mod sentinel_gates;

use crate::kernel::phasestack::cert::DeveloperCertificateStore;
use crate::kernel::phasestack::hooks::PhaseHookBus;
use crate::kernel::phasestack::ledger::AppLedger;
use crate::kernel::phasestack::registry::AppRegistry;
use crate::kernel::phasestack::revocation::PackageRevocationStore;
use crate::kernel::phasestack::supervisor::SupervisorPolicy;

pub struct PhaseStack {
    pub certs: DeveloperCertificateStore,
    pub revocation: PackageRevocationStore,
    pub registry: AppRegistry,
    pub hooks: PhaseHookBus,
    pub ledger: AppLedger,
    pub supervisor: SupervisorPolicy,
}

impl PhaseStack {
    pub fn new() -> Self {
        Self {
            certs: DeveloperCertificateStore::new(),
            revocation: PackageRevocationStore::new(),
            registry: AppRegistry::new(),
            hooks: PhaseHookBus::new(),
            ledger: AppLedger::new(),
            supervisor: SupervisorPolicy::default(),
        }
    }

    pub fn registry_root(&self) -> crate::kernel::hash::StateHash {
        install::app_registry_root(self)
    }
}

impl Default for PhaseStack {
    fn default() -> Self {
        Self::new()
    }
}
