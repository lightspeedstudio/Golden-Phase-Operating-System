//! Sentinel-gated launch, stop, and revoke. Spawn happens only after Allow.

use crate::kernel::crypto::phase_hash::ph_code;
use crate::kernel::phasestack::ledger::{LedgerEvent, LedgerKind};
use crate::kernel::phasestack::loader::PhaseAppLoader;
use crate::kernel::phasestack::native;
use crate::kernel::phasestack::types::{
    AppExecutionKind, AppId, AppState, LaunchResult, PhaseStackError, VerifiedInstallation,
    PKG_TYPE_PLUGIN,
};
use crate::kernel::phasestack::vm_runtime::PhaseVmRuntime;
use crate::kernel::state_proof::ObjectKind;
use crate::kernel::state_root::SubsystemKind;
use crate::kernel::Kernel;

fn mint_launch_nonce(kernel: &Kernel, app_id: AppId, sequence: u64) -> [u8; 16] {
    let tick = kernel.clock.snapshot().chronos_tick;
    #[cfg(feature = "phasesentinel")]
    {
        use crate::kernel::crypto::phase_hash::ph_sentinel_launch_receipt;
        let h = ph_sentinel_launch_receipt(&[&app_id, &tick.to_le_bytes(), &sequence.to_le_bytes()]);
        let mut n = [0u8; 16];
        n.copy_from_slice(&h[..16]);
        n
    }
    #[cfg(not(feature = "phasesentinel"))]
    {
        let mut n = [0u8; 16];
        n[..8].copy_from_slice(&tick.to_le_bytes());
        n[8..].copy_from_slice(&sequence.to_le_bytes());
        let _ = (kernel, app_id);
        n
    }
}

fn launch_result(
    pid: u32,
    retired: usize,
    output: crate::platform::Vec<u8>,
    byte_output: crate::platform::Vec<u8>,
    inst: &VerifiedInstallation,
    nonce: [u8; 16],
) -> LaunchResult {
    LaunchResult {
        pid,
        retired_instructions: retired,
        output,
        byte_output,
        launch_nonce: nonce,
        payload_root: inst.package().code_hash(),
        manifest_root: inst.package().manifest_hash(),
        registry_generation: inst.registry_generation(),
    }
}

#[cfg(feature = "phasesentinel")]
fn map_refusal(r: crate::kernel::sentinel::SentinelRefusal) -> PhaseStackError {
    use crate::kernel::sentinel::SentinelRefusal::*;
    match r {
        RegistryMissing => PhaseStackError::RegistryMissing,
        NotInstalled => PhaseStackError::NotInstalled,
        PackageRevoked => PhaseStackError::Revoked,
        PackageQuarantined => PhaseStackError::Quarantined,
        SignerUnknown => PhaseStackError::SignerUnknown,
        SignerRetired => PhaseStackError::RetiredSigner,
        ManifestRootMismatch => PhaseStackError::ManifestRootMismatch,
        PayloadRootMismatch => PhaseStackError::PayloadRootMismatch,
        RegistryGenerationMismatch => PhaseStackError::RegistryGenerationMismatch,
        InstallSequenceMismatch => PhaseStackError::InstallSequenceMismatch,
        CapabilityEscalation => PhaseStackError::CapabilityEscalation,
        MemoryEnvelopeInvalid => PhaseStackError::MemoryEnvelopeInvalid,
        EntryNotDeclared => PhaseStackError::EntryNotDeclared,
        NonceReplayed => PhaseStackError::NonceReplayed,
        PermitConsumed => PhaseStackError::PermitConsumed,
        KernelStateInvalid => PhaseStackError::KernelStateInvalid,
        ProcessAlreadyRunning => PhaseStackError::ProcessAlreadyRunning,
        BoundExceeded => PhaseStackError::BoundExceeded,
    }
}

#[cfg(feature = "phasesentinel")]
pub fn record_launch_event(
    kernel: &mut Kernel,
    op: &str,
    ticket: &crate::kernel::phasestack::ticket::VerifiedLaunchTicket,
    pid: u32,
    decision: &str,
) {
    use crate::core::gphase_core::PhaseState;
    use crate::kernel::crypto::phase_hash::ph_sentinel_launch_receipt;
    let prev = kernel.statechain.kernel_root();
    let tick = kernel.clock.snapshot().chronos_tick;
    let kairos = kernel.clock.snapshot().kairos_tick;
    let payload = ph_sentinel_launch_receipt(&[
        &prev,
        &ticket.package_id(),
        &ticket.package_version().encode(),
        &ticket.signer_key_id(),
        &ticket.manifest_root(),
        &ticket.payload_root(),
        &ticket.registry_generation().to_le_bytes(),
        &ticket.requested_caps().to_le_bytes(),
        &ticket.granted_caps().to_le_bytes(),
        &pid.to_le_bytes(),
        &ticket.launch_nonce(),
        decision.as_bytes(),
        &tick.to_le_bytes(),
        &kairos.to_le_bytes(),
    ]);
    let _ = kernel.statechain.record_transition(
        ObjectKind::Syscall,
        crate::kernel::sentinel::SENTINEL_EVIDENCE_OBJECT_ID,
        SubsystemKind::Syscall,
        PhaseState::Phi1,
        PhaseState::Phi2,
        op,
        None,
        &payload,
    );
}

#[cfg(feature = "phasesentinel")]
pub fn live_view(
    kernel: &Kernel,
    inst: &VerifiedInstallation,
) -> crate::kernel::sentinel::LiveLaunchView {
    use crate::kernel::phasestack::cert::is_retired_research_ca;
    let signer = inst.package().publisher_key_id();
    let cert_hash = inst.package().developer_cert_hash();
    crate::kernel::sentinel::LiveLaunchView {
        present: true,
        state: inst.state(),
        package_id: inst.package().app_id(),
        package_version_major: inst.package().version().major,
        package_version_minor: inst.package().version().minor,
        package_version_patch: inst.package().version().patch,
        signer_key_id: signer,
        developer_cert_hash: cert_hash,
        manifest_root: inst.package().manifest_hash(),
        payload_root: inst.package().code_hash(),
        registry_generation: inst.registry_generation(),
        install_sequence: inst.install_sequence(),
        requested_caps: inst.package().requested_caps(),
        granted_caps: inst.granted_caps(),
        mem_bytes: inst.package().envelope().mem_bytes,
        entry_point: inst.package().envelope().entry_point,
        signer_known: kernel.phasestack.certs.known_subject_key(&signer)
            || kernel.phasestack.certs.get(cert_hash).is_some(),
        signer_retired: is_retired_research_ca(&signer),
        signer_revoked: kernel.phasestack.revocation.cert_revoked(cert_hash),
        package_revoked: kernel
            .phasestack
            .revocation
            .package_revoked(inst.package().package_root()),
        policy_caps: kernel.sentinel.policy.max_rights,
        kernel_permits_launch: true,
    }
}

pub fn launch(
    kernel: &mut Kernel,
    app_id: AppId,
    fuel: usize,
) -> Result<LaunchResult, PhaseStackError> {
    let inst = kernel
        .phasestack
        .registry
        .get(app_id)
        .cloned()
        .ok_or(PhaseStackError::RegistryMissing)?;
    match inst.state() {
        AppState::Quarantined => return Err(PhaseStackError::Quarantined),
        AppState::Revoked => return Err(PhaseStackError::Revoked),
        AppState::Running => return Err(PhaseStackError::ProcessAlreadyRunning),
        AppState::Installed | AppState::Stopped => {}
        AppState::Verified => return Err(PhaseStackError::NotInstalled),
    }
    kernel.phasestack.revocation.check_package(
        inst.package().package_root(),
        inst.package().developer_cert_hash(),
        inst.package().developer_cert_hash(),
        0,
    )?;
    let code_path = format!("{}/code.phase", inst.store_path());
    let loaded_bytes = kernel
        .fs
        .read_file(&code_path)
        .map_err(|_| PhaseStackError::Fs)?;
    if ph_code(&loaded_bytes) != inst.package().code_hash() {
        kernel
            .phasestack
            .registry
            .set_state(app_id, AppState::Quarantined)?;
        let _ = kernel.phasestack.registry.bump_generation();
        kernel.phasestack.ledger.record(LedgerEvent {
            kind: LedgerKind::Quarantine,
            app_id,
            package_root: inst.package().package_root(),
            tick: kernel.clock.snapshot().chronos_tick,
            payload_hash: ph_code(&loaded_bytes),
        });
        return Err(PhaseStackError::PayloadRootMismatch);
    }
    if ph_code(inst.package().code()) != inst.package().code_hash() {
        return Err(PhaseStackError::PayloadRootMismatch);
    }
    let loaded = PhaseAppLoader::load(&inst)?;
    if ph_code(&loaded.code) != inst.package().code_hash() {
        return Err(PhaseStackError::PayloadRootMismatch);
    }
    let nonce = mint_launch_nonce(kernel, app_id, inst.install_sequence());

    #[cfg(feature = "phasesentinel")]
    let permit = {
        use crate::kernel::phasestack::ticket::VerifiedLaunchTicket;
        use crate::kernel::sentinel::SentinelLaunchDecision;
        let ticket = VerifiedLaunchTicket::from_installation(&inst, nonce);
        record_launch_event(kernel, "SentinelLaunchRequested", &ticket, 0, "requested");
        let view = live_view(kernel, &inst);
        match kernel.sentinel.authorize_verified_ticket(&ticket, &view) {
            SentinelLaunchDecision::Allow(p) => {
                record_launch_event(kernel, "SentinelLaunchAllowed", &ticket, 0, "Allow");
                let gen = inst.registry_generation();
                kernel
                    .sentinel
                    .consume_permit(p, gen, false)
                    .map_err(map_refusal)?
            }
            SentinelLaunchDecision::Refuse(r) => {
                record_launch_event(kernel, "SentinelLaunchRefused", &ticket, 0, r.as_str());
                return Err(map_refusal(r));
            }
            SentinelLaunchDecision::Isolate(i) => {
                record_launch_event(
                    kernel,
                    "SentinelLaunchIsolated",
                    &ticket,
                    0,
                    i.reason.as_str(),
                );
                return Err(map_refusal(i.reason));
            }
        }
    };

    #[cfg(feature = "phasesentinel")]
    let nonce = permit.launch_nonce();

    let pid = kernel.spawn_process(inst.package().name());
    if kernel.wake(pid).is_err() {
        let _ = kernel.kill_process(pid);
        return Err(PhaseStackError::KernelStateInvalid);
    }
    if kernel.schedule(pid).is_err() {
        let _ = kernel.kill_process(pid);
        return Err(PhaseStackError::KernelStateInvalid);
    }

    #[cfg(feature = "phasesentinel")]
    {
        use crate::kernel::phasestack::ticket::VerifiedLaunchTicket;
        let ticket = VerifiedLaunchTicket::from_installation(&inst, nonce);
        record_launch_event(kernel, "ProcessSpawned", &ticket, pid, "spawned");
        let _ = &permit;
    }

    if ph_code(&loaded.code) != inst.package().code_hash() {
        let _ = kernel.kill_process(pid);
        return Err(PhaseStackError::PayloadRootMismatch);
    }

    let report = match loaded.exec_kind {
        AppExecutionKind::Native => {
            if inst.package().package_type() == PKG_TYPE_PLUGIN {
                let _ = kernel.kill_process(pid);
                let img = native::parse_elf(&loaded.code)?;
                return native::execute_native_plugin(&img).map(|_| LaunchResult {
                    pid,
                    retired_instructions: 0,
                    output: crate::platform::Vec::new(),
                    byte_output: crate::platform::Vec::new(),
                    launch_nonce: nonce,
                    payload_root: inst.package().code_hash(),
                    manifest_root: inst.package().manifest_hash(),
                    registry_generation: inst.registry_generation(),
                });
            }
            #[cfg(all(target_arch = "arm", feature = "mmu-app-isolation"))]
            {
                if !native::is_elf(&loaded.code) {
                    match crate::arch::armv7::isolation::run_raw_a32(
                        kernel,
                        &loaded.code,
                        pid,
                        inst.package().package_root(),
                    ) {
                        Ok(true) => {
                            return Ok(launch_result(
                                pid,
                                1,
                                crate::platform::Vec::new(),
                                crate::platform::Vec::new(),
                                &inst,
                                nonce,
                            ));
                        }
                        Ok(false) => return Err(PhaseStackError::Quarantined),
                        Err(_) => {
                            let _ = kernel.kill_process(pid);
                            return Err(PhaseStackError::NativeIsolationUnavailable);
                        }
                    }
                }
                let img = native::parse_elf(&loaded.code)?;
                match native::execute(
                    &img,
                    &loaded.code,
                    kernel,
                    pid,
                    inst.package().package_root(),
                ) {
                    Ok(true) => {
                        return Ok(launch_result(
                            pid,
                            1,
                            crate::platform::Vec::new(),
                            crate::platform::Vec::new(),
                            &inst,
                            nonce,
                        ));
                    }
                    Ok(false) => return Err(PhaseStackError::Quarantined),
                    Err(e) => {
                        let _ = kernel.kill_process(pid);
                        return Err(e);
                    }
                }
            }
            #[cfg(not(all(target_arch = "arm", feature = "mmu-app-isolation")))]
            {
            let _ = kernel.kill_process(pid);
            let img = native::parse_elf(&loaded.code)?;
            let payload_root = inst.package().code_hash();
            let manifest_root = inst.package().manifest_hash();
            let gen = inst.registry_generation();
            return native::execute(
                &img,
                &loaded.code,
                kernel,
                pid,
                inst.package().package_root(),
            )
            .map(|_| LaunchResult {
                pid,
                retired_instructions: 0,
                output: crate::platform::Vec::new(),
                byte_output: crate::platform::Vec::new(),
                launch_nonce: nonce,
                payload_root,
                manifest_root,
                registry_generation: gen,
            });
            }
        }
        AppExecutionKind::PhaseVm => {
            let mut rt = PhaseVmRuntime::new(&loaded.code, inst.package().envelope().mem_bytes)?;
            rt.inject_launch_nonce(&nonce)?;
            match rt.execute(fuel) {
                Ok(report) => report,
                Err(e) => {
                    let _ = kernel.kill_process(pid);
                    return Err(e);
                }
            }
        }
    };
    if report.retired == 0 {
        let _ = kernel.kill_process(pid);
        return Err(PhaseStackError::NoInstruction);
    }
    if kernel.syscall(pid).is_err() {
        let _ = kernel.kill_process(pid);
        return Err(PhaseStackError::KernelStateInvalid);
    }
    kernel
        .phasestack
        .registry
        .set_state(app_id, AppState::Running)?;
    kernel
        .phasestack
        .registry
        .set_process_id(app_id, Some(pid))?;
    kernel.phasestack.ledger.record(LedgerEvent {
        kind: LedgerKind::Launch,
        app_id,
        package_root: inst.package().package_root(),
        tick: kernel.clock.snapshot().chronos_tick,
        payload_hash: inst.package().code_hash(),
    });
    #[cfg(feature = "phasesentinel")]
    {
        use crate::kernel::phasestack::ticket::VerifiedLaunchTicket;
        let ticket = VerifiedLaunchTicket::from_installation(&inst, nonce);
        record_launch_event(kernel, "ApplicationOutput", &ticket, pid, "output");
    }
    Ok(launch_result(
        pid,
        report.retired,
        report.output,
        report.byte_output,
        &inst,
        nonce,
    ))
}

pub fn stop(kernel: &mut Kernel, app_id: AppId) -> Result<(), PhaseStackError> {
    let inst = kernel
        .phasestack
        .registry
        .get(app_id)
        .cloned()
        .ok_or(PhaseStackError::NotFound)?;
    if let Some(pid) = inst.process_id() {
        match kernel.get_process(pid).map(|p| p.state) {
            Ok(crate::core::gphase_core::PhaseState::Phi3) => {
                let _ = kernel.exit(pid);
            }
            _ => {
                let _ = kernel.kill_process(pid);
            }
        }
    }
    kernel.phasestack.registry.set_process_id(app_id, None)?;
    kernel
        .phasestack
        .registry
        .set_state(app_id, AppState::Installed)?;
    #[cfg(feature = "phasesentinel")]
    {
        kernel
            .sentinel
            .permit_table_mut()
            .invalidate_generation(inst.registry_generation());
        use crate::kernel::phasestack::ticket::VerifiedLaunchTicket;
        let ticket = VerifiedLaunchTicket::from_installation(&inst, [0u8; 16]);
        record_launch_event(kernel, "ApplicationStopped", &ticket, 0, "stopped");
    }
    Ok(())
}

pub fn revoke(kernel: &mut Kernel, app_id: AppId) -> Result<(), PhaseStackError> {
    let inst = kernel
        .phasestack
        .registry
        .get(app_id)
        .cloned()
        .ok_or(PhaseStackError::NotFound)?;
    let _ = stop(kernel, app_id);
    let previous = kernel.phasestack.revocation.clone();
    kernel
        .phasestack
        .revocation
        .revoke_package(inst.package().package_root());
    if let Err(e) = crate::kernel::phasestack::install::persist_revocation(kernel) {
        kernel.phasestack.revocation = previous;
        return Err(e);
    }
    if let Some(row) = kernel.phasestack.registry.get_mut(app_id) {
        row.set_granted_caps(0);
        row.set_state(AppState::Revoked);
    }
    let _ = kernel.phasestack.registry.bump_generation();
    kernel.phasestack.ledger.record(LedgerEvent {
        kind: LedgerKind::Revoke,
        app_id,
        package_root: inst.package().package_root(),
        tick: kernel.clock.snapshot().chronos_tick,
        payload_hash: inst.package().package_root(),
    });
    #[cfg(feature = "phasesentinel")]
    {
        use crate::kernel::phasestack::ticket::VerifiedLaunchTicket;
        let ticket = VerifiedLaunchTicket::from_installation(&inst, [0u8; 16]);
        record_launch_event(kernel, "ApplicationRevoked", &ticket, 0, "revoked");
    }
    Ok(())
}

#[cfg(feature = "phasesentinel-research")]
pub const RESEARCH_UNLOCK_TOKEN: &str = "PHASESENTINEL-LIVE-INJECT";

#[cfg(feature = "phasesentinel-research")]
pub fn research_bump_generation(
    kernel: &mut Kernel,
    token: &str,
) -> Result<u64, PhaseStackError> {
    if token != RESEARCH_UNLOCK_TOKEN {
        return Err(PhaseStackError::AuthorityDenied);
    }
    kernel.phasestack.registry.bump_generation()
}
