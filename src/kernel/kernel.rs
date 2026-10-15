// Copyright (c) 2024–2026 Richard Careaga, Metatron Research.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial

//! Thin facade wiring scheduler and memory; no new algebra.

use crate::core::gphase_core::PhaseState;
use crate::fs::PhaseFs;
#[cfg(all(feature = "baremetal", not(feature = "std")))]
use alloc::boxed::Box;
use crate::kernel::anom::AnomController;
use crate::kernel::authority::{AuthorityState, PublicPhaseView};
#[cfg(all(feature = "captable", any(feature = "std", feature = "baremetal")))]
use crate::kernel::captable::CapabilityTable;
use crate::kernel::curvature::CurvatureLog;
use crate::kernel::event_log::{PageId, ProcessId, TopologyEvent};
#[cfg(all(feature = "extmap", any(feature = "std", feature = "baremetal")))]
use crate::kernel::extmap::Extmap;
#[cfg(all(feature = "identity", any(feature = "std", feature = "baremetal")))]
use crate::kernel::identity::Identity;
#[cfg(all(feature = "phasestack", any(feature = "std", feature = "baremetal")))]
use crate::kernel::phasestack::PhaseStack;
#[cfg(all(feature = "phasesentinel", any(feature = "std", feature = "baremetal")))]
use crate::kernel::sentinel::PhaseSentinel;
#[cfg(all(feature = "ipc", any(feature = "std", feature = "baremetal")))]
use crate::kernel::ipc::Ipc;
use crate::kernel::phase_consensus::{PhaseAccelerator, PhaseConsensus};
use crate::kernel::memory::{Memory, MemoryError};
#[cfg(all(feature = "netsess", any(feature = "std", feature = "baremetal")))]
use crate::kernel::netsess::Netsess;
use crate::kernel::network::NetworkInterfaceStatus;
#[cfg(any(feature = "std", feature = "baremetal"))]
use crate::kernel::phase_cache::PhaseCacheController;
use crate::kernel::phase_index::PhaseIndexController;
use crate::kernel::phase_math::PhaseMathController;
use crate::kernel::phasenet::PhaseNetController;
#[cfg(all(feature = "pmm", any(feature = "std", feature = "baremetal")))]
use crate::kernel::pmm::PhysicalMemoryManager;
use crate::kernel::process::Process;
#[cfg(all(feature = "sched2", any(feature = "std", feature = "baremetal")))]
use crate::kernel::sched2::Sched2;
use crate::kernel::scheduler::Scheduler;
use crate::kernel::state_domain::StateDomainVm;
#[cfg(any(feature = "std", feature = "baremetal"))]
use crate::kernel::state_proof::ObjectKind;
#[cfg(any(feature = "std", feature = "baremetal"))]
use crate::kernel::state_root::SubsystemKind;
#[cfg(any(feature = "std", feature = "baremetal"))]
use crate::kernel::statechain::StatechainLedger;
#[cfg(any(feature = "std", feature = "baremetal"))]
use crate::kernel::checkpoint_auth::CheckpointAuthController;
use crate::kernel::trace::{KernelTrace, TraceRecord};
#[cfg(all(feature = "txlog", any(feature = "std", feature = "baremetal")))]
use crate::kernel::txlog::Txlog;
use crate::platform::{HashMap, Vec};
use crate::stateclock::{self, StateClock};

/// Default `PhaseCache` capacity. Small relative to the host-research
/// prototype's 2,048-8,192 entry benchmark caches — this cache targets a
/// single kernel's live path-resolution working set, not a synthetic
/// large-object-count workload.
#[cfg(any(feature = "std", feature = "baremetal"))]
const PHASE_CACHE_CAPACITY: usize = 64;

pub struct Kernel {
    pub scheduler: Scheduler,
    pub memory: Memory,
    /// Real physical-frame buddy/slab allocator — see `kernel::pmm`. Additive:
    /// `memory` (the logical phase-tag page model) remains the authorization
    /// layer above it. Off by default; see `fpga/zynq/evidence/PMM_STATUS_*.md`.
    #[cfg(all(feature = "pmm", any(feature = "std", feature = "baremetal")))]
    pub physical_memory: PhysicalMemoryManager,
    /// Logical `PageId` (from `memory.alloc_page`) -> physical address backing
    /// it, populated only when `pmm` is enabled.
    #[cfg(all(feature = "pmm", any(feature = "std", feature = "baremetal")))]
    page_frames: HashMap<PageId, usize>,
    pub fs: PhaseFs,
    /// Additive (state, Kairos-sector, Chronos-bucket) index over `fs`'s
    /// inodes — see `kernel::phase_index` and `kernel::syscall`'s FS_* handlers.
    pub phase_index: PhaseIndexController,
    /// Trusted, two-clock cache over `fs` path resolution, gated against
    /// `statechain`'s checkpoint chain — see `kernel::phase_cache` and
    /// `kernel::syscall`'s FS_* handlers.
    #[cfg(any(feature = "std", feature = "baremetal"))]
    pub phase_cache: PhaseCacheController,
    /// Generational object/capability table backing `FS_OPEN`/`FS_CLOSE` —
    /// see `kernel::captable`. Off by default; see
    /// `fpga/zynq/evidence/CAPTABLE_STATUS_*.md`.
    #[cfg(all(feature = "captable", any(feature = "std", feature = "baremetal")))]
    pub captable: CapabilityTable,
    /// Software DMA containment. Not an IOMMU.
    #[cfg(any(feature = "std", feature = "baremetal"))]
    pub dma: crate::kernel::dma_contain::DmaContainment,
    /// Ordinary run-state / weighted-fairness / timer-wheel scheduler,
    /// parallel to (and never rewriting) `scheduler`'s canonical `PhaseState`
    /// transitions — see `kernel::sched2`. Off by default; see
    /// `fpga/zynq/evidence/SCHED2_STATUS_*.md`.
    #[cfg(all(feature = "sched2", any(feature = "std", feature = "baremetal")))]
    pub sched2: Sched2,
    /// Bounded message queues, gated by `captable` capabilities and using
    /// `sched2` to block a receiver on an empty queue — see `kernel::ipc`.
    /// Off by default; see `fpga/zynq/evidence/IPC_STATUS_*.md`.
    #[cfg(all(feature = "ipc", any(feature = "std", feature = "baremetal")))]
    pub ipc: Ipc,
    /// Real, fixed-size block allocator and multi-block extent map over
    /// `fs`'s inode content — see `kernel::extmap`. Additive: `fs`'s own
    /// single-legacy-block storage remains untouched underneath; only
    /// `kernel::syscall`'s FS_WRITE_FILE/FS_READ_FILE/FS_RM handlers also
    /// route through this when the feature is on. Off by default; see
    /// `fpga/zynq/evidence/EXTMAP_STATUS_*.md`.
    #[cfg(all(feature = "extmap", any(feature = "std", feature = "baremetal")))]
    pub extmap: Extmap,
    /// Real, bounded, multi-op transaction log tying batched
    /// `write_file`/`mkdir`/`rm` mutations to the real `StatechainLedger` —
    /// see `kernel::txlog`. RAM software-reset journal, not the SD WAL.
    /// Persist uses this only when `phasefs-journal` is off, or when board
    /// WAL cannot start; WAL success skips `txlog`. Off by default; see
    /// `fpga/zynq/evidence/TXLOG_STATUS_*.md`.
    #[cfg(all(feature = "txlog", any(feature = "std", feature = "baremetal")))]
    pub txlog: Txlog,
    /// Host-side RAM journal (std only). Board uses SD partition 1 via
    /// `board::zynq7000::journal_sd`. Off by default.
    #[cfg(all(feature = "phasefs-journal", feature = "std"))]
    pub fs_journal: crate::fs::journal::PhaseFsJournal<crate::fs::blockdev::RamBlockDev>,
    #[cfg(all(feature = "phasefs-journal", any(feature = "std", feature = "baremetal")))]
    pub journal_crash: crate::fs::journal::CrashPoint,
    #[cfg(all(feature = "phasefs-journal", any(feature = "std", feature = "baremetal")))]
    pub journal_last: crate::fs::journal::RecoverReport,
    #[cfg(all(feature = "phasefs-journal", any(feature = "std", feature = "baremetal")))]
    pub journal_backend: crate::fs::journal::JournalBackend,
    /// Bounded network session table + software descriptor ring. Live GEM
    /// RX BD takes record `KernelReady` here after hardware released the
    /// BD; this field does not own GEM DMA rings or store `HwOwned`. See
    /// `kernel::netsess`. Additive: no existing net path is replaced.
    /// Off by default; see `fpga/zynq/evidence/NETSESS_STATUS_*.md`.
    #[cfg(all(feature = "netsess", any(feature = "std", feature = "baremetal")))]
    pub netsess: Netsess,
    /// User identity, password, certificate, and authority subsystem — see
    /// `kernel::identity`. Additive: does not replace `authority`
    /// (`AuthorityState`, the global Root/Anom mode label) or `captable`
    /// (object capabilities); it is the new principal/credential layer
    /// above them. Off by default; see the identity build plan's evidence
    /// docs once written.
    #[cfg(all(feature = "identity", any(feature = "std", feature = "baremetal")))]
    pub identity: Identity,
    /// Verified application runtime. Off by default; see `phasestack` feature.
    #[cfg(all(feature = "phasestack", any(feature = "std", feature = "baremetal")))]
    pub phasestack: PhaseStack,
    /// PhaseSentinel launch/observe/quarantine. Off by default.
    #[cfg(all(feature = "phasesentinel", any(feature = "std", feature = "baremetal")))]
    pub sentinel: PhaseSentinel,
    pub network: NetworkInterfaceStatus,
    pub trace: KernelTrace,
    pub clock: StateClock,
    pub authority: AuthorityState,
    pub anom: AnomController,
    /// Avenue 8 causal-curvature watchdog: per-tick reflection-defect log.
    pub curvature: CurvatureLog,
    #[cfg(any(feature = "std", feature = "baremetal"))]
    pub object_protection: crate::kernel::anom::ObjectProtectionController,
    pub phase_math: PhaseMathController,
    pub state_domain: StateDomainVm,
    #[cfg(any(feature = "std", feature = "baremetal"))]
    pub phaselink: crate::kernel::phaselink::PhaseLinkController,
    pub phasenet: PhaseNetController,
    pub phi_stability: crate::kernel::phi_stability::PhiStabilityController,
    #[cfg(any(feature = "std", feature = "baremetal"))]
    pub transport: Option<crate::kernel::phase_transport::TransportController>,
    #[cfg(feature = "std")]
    pub phase_benchmark: crate::kernel::phase_benchmark::PhaseBenchmarkController,
    #[cfg(any(feature = "std", feature = "baremetal"))]
    pub ai: crate::kernel::ai::AiBridgeController,
    #[cfg(feature = "std")]
    pub ssh: crate::kernel::remote_shell::types::SshController,
    #[cfg(any(feature = "std", feature = "baremetal"))]
    pub statechain: StatechainLedger,
    /// Secret-backed GPC1 checkpoints. Default backend is AUTH_UNAVAILABLE.
    /// V1/V2 `statechain` HMAC stays integrity-only.
    #[cfg(any(feature = "std", feature = "baremetal"))]
    pub checkpoint_auth: CheckpointAuthController,
    /// Gate-4 PhaseGuard observer (read-only). Present only with `phaseguard-research`.
    #[cfg(feature = "phaseguard-research")]
    pub phaseguard: crate::stateclock::phaseguard_live::PhaseGuardObserver,
    /// Optional (prev, current, proposed) for the next Kairos observe hook.
    #[cfg(feature = "phaseguard-research")]
    pub phaseguard_pending: Option<(PhaseState, PhaseState, PhaseState)>,
    /// Host default is Emulated. Board attaches G1P after a successful identity probe.
    pub consensus: PhaseConsensus,
}

impl Default for Kernel {
    fn default() -> Self {
        Self::new()
    }
}

impl Kernel {
    #[allow(unused_mut)]
    pub fn new() -> Self {
        let mut kernel = Self {
            scheduler: Scheduler::new(),
            memory: Memory::new(),
            #[cfg(all(feature = "pmm", any(feature = "std", feature = "baremetal")))]
            physical_memory: Self::new_physical_memory_manager(),
            #[cfg(all(feature = "pmm", any(feature = "std", feature = "baremetal")))]
            page_frames: HashMap::new(),
            fs: PhaseFs::new(),
            phase_index: PhaseIndexController::new(),
            #[cfg(any(feature = "std", feature = "baremetal"))]
            phase_cache: PhaseCacheController::new(PHASE_CACHE_CAPACITY),
            #[cfg(all(feature = "captable", any(feature = "std", feature = "baremetal")))]
            captable: CapabilityTable::new(),
            #[cfg(any(feature = "std", feature = "baremetal"))]
            dma: crate::kernel::dma_contain::DmaContainment::new(),
            #[cfg(all(feature = "sched2", any(feature = "std", feature = "baremetal")))]
            sched2: Sched2::new(),
            #[cfg(all(feature = "ipc", any(feature = "std", feature = "baremetal")))]
            ipc: Ipc::new(),
            #[cfg(all(feature = "extmap", any(feature = "std", feature = "baremetal")))]
            extmap: Extmap::default(),
            #[cfg(all(feature = "txlog", any(feature = "std", feature = "baremetal")))]
            txlog: Txlog::default(),
            #[cfg(all(feature = "phasefs-journal", feature = "std"))]
            fs_journal: crate::fs::journal::PhaseFsJournal::format_ram(&PhaseFs::new())
                .expect("ram journal format"),
            #[cfg(all(feature = "phasefs-journal", any(feature = "std", feature = "baremetal")))]
            journal_crash: crate::fs::journal::CrashPoint::None,
            #[cfg(all(feature = "phasefs-journal", any(feature = "std", feature = "baremetal")))]
            journal_last: crate::fs::journal::RecoverReport::default(),
            #[cfg(all(feature = "phasefs-journal", any(feature = "std", feature = "baremetal")))]
            journal_backend: {
                #[cfg(feature = "std")]
                {
                    crate::fs::journal::JournalBackend::RamFallback
                }
                #[cfg(not(feature = "std"))]
                {
                    crate::fs::journal::JournalBackend::Absent
                }
            },
            #[cfg(all(feature = "netsess", any(feature = "std", feature = "baremetal")))]
            netsess: Netsess::default(),
            #[cfg(all(feature = "identity", any(feature = "std", feature = "baremetal")))]
            identity: Identity::default(),
            #[cfg(all(feature = "phasestack", any(feature = "std", feature = "baremetal")))]
            phasestack: PhaseStack::new(),
            #[cfg(all(feature = "phasesentinel", any(feature = "std", feature = "baremetal")))]
            sentinel: PhaseSentinel::new(),
            network: NetworkInterfaceStatus::default(),
            trace: KernelTrace::default(),
            clock: StateClock::new(),
            authority: AuthorityState::default(),
            anom: AnomController::new(),
            curvature: CurvatureLog::new(),
            #[cfg(any(feature = "std", feature = "baremetal"))]
            object_protection: crate::kernel::anom::ObjectProtectionController::new(),
            phase_math: PhaseMathController::new(),
            state_domain: StateDomainVm::new(),
            #[cfg(any(feature = "std", feature = "baremetal"))]
            phaselink: crate::kernel::phaselink::PhaseLinkController::new(),
            phasenet: PhaseNetController::new(),
            phi_stability: crate::kernel::phi_stability::PhiStabilityController::new(),
            #[cfg(any(feature = "std", feature = "baremetal"))]
            transport: None,
            #[cfg(feature = "std")]
            phase_benchmark: crate::kernel::phase_benchmark::PhaseBenchmarkController::new(),
            #[cfg(any(feature = "std", feature = "baremetal"))]
            ai: crate::kernel::ai::AiBridgeController::new(),
            #[cfg(feature = "std")]
            ssh: crate::kernel::remote_shell::types::SshController::new(),
            #[cfg(any(feature = "std", feature = "baremetal"))]
            statechain: StatechainLedger::new(),
            #[cfg(any(feature = "std", feature = "baremetal"))]
            checkpoint_auth: CheckpointAuthController::new(),
            #[cfg(feature = "phaseguard-research")]
            phaseguard: crate::stateclock::phaseguard_live::PhaseGuardObserver::new(),
            #[cfg(feature = "phaseguard-research")]
            phaseguard_pending: None,
            consensus: PhaseConsensus::emulated(),
        };
        #[cfg(all(feature = "identity", any(feature = "std", feature = "baremetal")))]
        {
            kernel.authority.boot_session_root =
                crate::kernel::boot_session::compute_boot_session_root(
                    &kernel.statechain.equation_root(),
                    1,
                    &kernel.statechain.kernel_root(),
                    0,
                );
        }
        kernel
    }

    /// Bind the GEM0 live session and (on PZ7020) the RX-take hook.
    /// Call once after `Kernel` is in its final location — `Runtime::new`
    /// returns by value, so this must not run inside `Kernel::new`.
    #[cfg(all(feature = "netsess", any(feature = "std", feature = "baremetal")))]
    pub fn bind_netsess_live(&mut self) {
        let root = self.authority.boot_session_root;
        let _ = self.netsess.bind_gem_session(root);
        #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
        self.netsess.bind_live();
    }

    #[cfg(any(feature = "std", feature = "baremetal"))]
    pub fn init_state_domain(&mut self) {
        let equation_root = self.statechain.equation_root();
        self.state_domain.init_with_equation(equation_root);
    }

    #[cfg(any(feature = "std", feature = "baremetal"))]
    pub fn change_domain_state(
        &mut self,
        object_key: &str,
        domain_id: &crate::kernel::state_domain::DomainId,
        new_state_label: &str,
        operation: &str,
    ) -> Result<
        crate::kernel::state_domain::StateChangeProof,
        crate::kernel::state_domain::StateChangeError,
    > {
        self.state_domain.change_virtual_state(
            &mut self.statechain,
            &self.clock,
            object_key,
            domain_id,
            new_state_label,
            operation,
        )
    }

    pub fn public_process_view(&self, pid: ProcessId) -> PublicPhaseView {
        let phase = self
            .get_process(pid)
            .ok()
            .map(|p| p.state)
            .unwrap_or(PhaseState::Phi0);
        #[cfg(any(feature = "std", feature = "baremetal"))]
        {
            let global_anom = self.authority.is_anom_protected();
            let obj_view = self
                .object_protection
                .public_view_for_process(pid, phase, global_anom);
            if matches!(
                obj_view,
                PublicPhaseView::AnomCommitment(_) | PublicPhaseView::Locked
            ) {
                return obj_view;
            }
            self.anom
                .public_view(ObjectKind::Process, pid, phase, &self.authority)
        }
        #[cfg(not(any(feature = "std", feature = "baremetal")))]
        {
            if self.authority.is_anom_protected() {
                PublicPhaseView::Locked
            } else {
                PublicPhaseView::Visible(phase)
            }
        }
    }

    pub fn get_clock(&self) -> &StateClock {
        &self.clock
    }

    /// Mutable access to the live StateClock — required by the executable
    /// fault-recovery transaction, which injects (research builds only) and
    /// restores real live clock state through this handle.
    pub fn get_clock_mut(&mut self) -> &mut StateClock {
        &mut self.clock
    }

    pub fn advance_chronos(&mut self) {
        let start = self.clock.events.len();
        let _ = self.clock.advance_chronos();
        self.phase_math.enqueue_clock(
            self.clock.proper_tick,
            self.clock.kairos_tick,
            self.clock.sector_id,
        );
        self.commit_new_clock_events(start);
        self.record_curvature_tick();
    }

    /// Sample the current lifetime reflection-defect count into the
    /// causal-curvature watchdog log and return the delta since the
    /// previous tick. Called once per `advance_chronos` boundary; also
    /// callable directly by instrumentation/demo code that wants to force
    /// an immediate sample (see `curvature inject`).
    pub fn record_curvature_tick(&mut self) -> u32 {
        let total = self.scheduler.reflection_count();
        self.curvature.record_tick(total)
    }

    #[cfg(feature = "std")]
    pub fn set_clock_dilation(&mut self, value: f64) -> Result<(), stateclock::ClockError> {
        let start = self.clock.events.len();
        self.clock.set_dilation_factor(value)?;
        self.commit_new_clock_events(start);
        Ok(())
    }

    #[cfg(not(feature = "std"))]
    pub fn set_clock_dilation(&mut self, value: f64) -> Result<(), stateclock::ClockError> {
        self.clock.set_dilation_factor(value).map(|_| ())
    }

    pub fn resync_clock(&mut self, reason: impl Into<crate::platform::String>) {
        let start = self.clock.events.len();
        self.clock.resync(reason);
        self.phase_math.enqueue_clock(
            self.clock.proper_tick,
            self.clock.kairos_tick,
            self.clock.sector_id,
        );
        self.commit_new_clock_events(start);
    }

    #[cfg(any(feature = "std", feature = "baremetal"))]
    fn commit_new_clock_events(&mut self, start: usize) {
        let end = self.clock.events.len();
        for i in start..end {
            let event = self.clock.events[i].clone();
            stateclock::commit_clock_event(self, &event);
        }
    }

    #[cfg(not(any(feature = "std", feature = "baremetal")))]
    fn commit_new_clock_events(&mut self, _start: usize) {}

    pub fn spawn(&mut self) -> ProcessId {
        self.spawn_process("")
    }

    pub fn spawn_process(&mut self, name: &str) -> ProcessId {
        let pid = self.scheduler.spawn_named(name);
        let proc_name = self
            .get_process(pid)
            .map(|p| p.name.clone())
            .unwrap_or_else(|_| name.into());
        self.trace.record(TraceRecord::Spawn {
            pid,
            name: proc_name.clone(),
        });
        stateclock::record_kairos_on_kernel(
            self,
            stateclock::KairosEventKind::Spawn,
            format!("{proc_name} pid {pid}"),
        );
        #[cfg(any(feature = "std", feature = "baremetal"))]
        self.record_process(pid, PhaseState::Phi0, PhaseState::Phi0, "spawn");
        pid
    }

    pub fn kill_process(&mut self, target: ProcessId) -> Result<(), &'static str> {
        if self.get_process(target).is_err() {
            self.trace.record(TraceRecord::Kill {
                pid: target,
                ok: false,
            });
            return Err("no such process");
        }
        if self.get_process(target)?.state == PhaseState::Phi0 {
            self.trace.record(TraceRecord::Kill {
                pid: target,
                ok: true,
            });
            return Ok(());
        }
        self.syscall(target).map_err(|_| "kill syscall rejected")?;
        self.exit(target).map_err(|_| "kill exit rejected")?;
        self.trace.record(TraceRecord::Kill {
            pid: target,
            ok: true,
        });
        stateclock::record_kairos_on_kernel(
            self,
            stateclock::KairosEventKind::Kill,
            format!("pid {target}"),
        );
        Ok(())
    }

    pub fn get_state(&self) -> KernelState {
        let running = self
            .process_ids()
            .into_iter()
            .filter(|pid| {
                self.get_process(*pid)
                    .map(|p| p.state == PhaseState::Phi2)
                    .unwrap_or(false)
            })
            .count();
        KernelState {
            process_count: self.process_ids().len(),
            page_count: self.memory.tag_report().len(),
            running_count: running,
        }
    }

    pub fn get_trace_log(&self) -> &[TraceRecord] {
        self.trace.recent()
    }

    #[cfg(any(feature = "std", feature = "baremetal"))]
    fn record_process(&mut self, pid: ProcessId, old: PhaseState, new: PhaseState, op: &str) {
        self.statechain
            .sync_reflection_defects(self.scheduler.reflection_count() as u64);
        self.phase_math.enqueue_transition(pid, old, new);
        self.phase_math.enqueue_proof(pid, self.statechain.tick());
        self.phase_math.enqueue_merkle_leaf(pid);
        self.phase_math.enqueue_watchdog(pid);
        if let Ok(proc) = self.get_process(pid) {
            if proc.history.len() >= 3 {
                let n = proc.history.len();
                self.phase_math.enqueue_trajectory(
                    proc.history[n - 3],
                    proc.history[n - 2],
                    proc.history[n - 1],
                );
            }
        }
        if crate::kernel::anom::object_protection::record_protected_transition(
            self, pid, old, new, op,
        ) {
            return;
        }
        crate::kernel::anom::protocol::mask_and_record_process(self, pid, old, new, op);
    }

    #[cfg(feature = "std")]
    fn record_memory(&mut self, page: PageId, old: PhaseState, new: PhaseState, op: &str) {
        self.phase_math.enqueue_transition(page, old, new);
        self.phase_math.enqueue_proof(page, self.statechain.tick());
        self.phase_math.enqueue_merkle_leaf(page);
        let _ = self.statechain.record_transition(
            ObjectKind::MemoryPage,
            page,
            SubsystemKind::Memory,
            old,
            new,
            op,
            None,
            &[],
        );
    }

    #[cfg(feature = "std")]
    pub fn record_syscall_event(&mut self, caller: ProcessId, nr: u32, operation: &str) {
        self.phase_math
            .enqueue_proof(caller, self.statechain.tick());
        self.phase_math.enqueue_merkle_leaf(caller);
        let mut payload = nr.to_le_bytes().to_vec();
        payload.extend_from_slice(operation.as_bytes());
        let _ = self.statechain.record_transition(
            ObjectKind::Syscall,
            caller,
            SubsystemKind::Syscall,
            PhaseState::Phi2,
            PhaseState::Phi3,
            operation,
            None,
            &payload,
        );
    }

    #[cfg(feature = "std")]
    pub fn record_fs_event(&mut self, path: &str, operation: &str) {
        use crate::kernel::state_proof::hash_payload;
        let digest = hash_payload(path.as_bytes());
        let object_id = u16::from_le_bytes([digest[0], digest[1]]) as u32;
        self.phase_math
            .enqueue_proof(object_id, self.statechain.tick());
        self.phase_math.enqueue_merkle_leaf(object_id);
        let mut payload = operation.as_bytes().to_vec();
        payload.extend_from_slice(path.as_bytes());
        let _ = self.statechain.record_transition(
            ObjectKind::File,
            object_id,
            SubsystemKind::Filesystem,
            PhaseState::Phi1,
            PhaseState::Phi2,
            operation,
            None,
            &payload,
        );
    }

    /// Rebuild PhaseIndex, drop PhaseCache, bind recovered fs_root into the
    /// filesystem ledger, and rebuild RAM extmap extents from recovered
    /// PhaseFs file bytes. Extmap is not its own WAL.
    #[cfg(all(feature = "phasefs-journal", any(feature = "std", feature = "baremetal")))]
    pub fn journal_post_recover(&mut self) {
        self.phase_index = PhaseIndexController::new();
        let stamps: crate::platform::Vec<(
            crate::fs::InodeId,
            PhaseState,
            u8,
            u64,
        )> = self
            .fs
            .iter_inodes()
            .map(|i| (i.id, i.tag, i.kairos_sector, i.chronos))
            .collect();
        for (id, tag, sector, chronos) in stamps {
            self.phase_index.insert(id, tag, sector, chronos);
        }
        self.phase_cache.invalidate_all();
        #[cfg(all(feature = "extmap", any(feature = "std", feature = "baremetal")))]
        {
            let file_ids: crate::platform::Vec<crate::fs::InodeId> = self
                .fs
                .iter_inodes()
                .filter(|i| i.kind == crate::fs::InodeKind::File)
                .map(|i| i.id)
                .collect();
            let mut files: crate::platform::Vec<(crate::fs::InodeId, crate::platform::Vec<u8>)> =
                crate::platform::Vec::new();
            for id in file_ids {
                if let Ok(data) = self.fs.read_file_by_inode(id) {
                    files.push((id, data));
                }
            }
            self.extmap = Extmap::default();
            for (id, data) in files {
                let _ = self.extmap.write_inode(id, &data);
            }
        }
        #[cfg(feature = "std")]
        self.record_fs_event("/", "fsj_recover");
        #[cfg(all(feature = "baremetal", not(feature = "std")))]
        {
            let _ = self.statechain.record_transition(
                ObjectKind::File,
                0,
                SubsystemKind::Filesystem,
                PhaseState::Phi1,
                PhaseState::Phi2,
                "fsj_recover",
                None,
                &self.journal_last.fs_root,
            );
        }
        #[cfg(feature = "phasestack")]
        {
            let _ = crate::kernel::phasestack::install::rebuild_from_store(self);
        }
        #[cfg(all(feature = "identity", any(feature = "std", feature = "baremetal")))]
        self.identity_restore_from_phasefs();
    }

    /// Load the identity snapshot from recovered PhaseFs. Missing file
    /// leaves RAM defaults (first boot).
    #[cfg(all(feature = "identity", any(feature = "std", feature = "baremetal")))]
    fn identity_restore_from_phasefs(&mut self) {
        let bytes = match self
            .fs
            .read_file(crate::kernel::identity::PHASEFS_STORE_PATH)
        {
            Ok(b) => b,
            Err(_) => return,
        };
        let _ = self.identity.restore_from_bytes(&bytes);
    }

    /// If WAL `/idstore.bin` is missing, import legacy FAT p0 `IDSTORE.BIN`
    /// once and persist it onto the journal.
    #[cfg(all(
        feature = "identity",
        feature = "baremetal",
        feature = "zynq-a9",
        not(feature = "std")
    ))]
    pub fn identity_import_fat_if_wal_empty(&mut self) {
        if self
            .fs
            .read_file(crate::kernel::identity::PHASEFS_STORE_PATH)
            .is_ok()
        {
            return;
        }
        let mut store = crate::kernel::identity::store::FatRootStore;
        match self.identity.load_from(&mut store) {
            Ok(()) => self.identity_checkpoint_statechain(),
            Err(_) => {}
        }
    }

    #[cfg(any(feature = "std", feature = "baremetal"))]
    pub fn record_vm_event(
        &mut self,
        vm_id: ProcessId,
        previous_state: PhaseState,
        requested_state: PhaseState,
        operation: &str,
        payload: &[u8],
    ) {
        self.phase_math
            .enqueue_transition(vm_id, previous_state, requested_state);
        self.phase_math.enqueue_proof(vm_id, self.statechain.tick());
        self.phase_math.enqueue_merkle_leaf(vm_id);
        let _ = self.statechain.record_transition(
            ObjectKind::Vm,
            vm_id,
            SubsystemKind::Vm,
            previous_state,
            requested_state,
            operation,
            None,
            payload,
        );
    }

    /// Checkpoint the identity subsystem's `AuthorityStateRoot` into the
    /// real `StatechainLedger` — the same "own internal chain, *also*
    /// checkpointed into the one real statechain" pattern
    /// `kernel::txlog::append_ledger_event` already established (see that
    /// function's docs). Keyed `(ObjectKind::Syscall, object_id =
    /// IDENTITY_LEDGER_CHECKPOINT_OBJECT_ID)`: the identity ledger is one
    /// global stream, not per-object. The id is a reserved small integer
    /// (`31`) because `StatechainLedger` indexes subsystem heads by
    /// `object_id`. Called after every real `identity_*` mutation below.
    #[cfg(all(feature = "identity", any(feature = "std", feature = "baremetal")))]
    fn identity_checkpoint_statechain(&mut self) {
        let sequence = self.identity.ledger().sequence();
        let state_root = self.identity.authority_state_root();
        let mut payload = sequence.to_le_bytes().to_vec();
        payload.extend_from_slice(&state_root);
        let _ = self.statechain.record_transition(
            crate::kernel::state_proof::ObjectKind::Syscall,
            crate::kernel::identity::IDENTITY_LEDGER_CHECKPOINT_OBJECT_ID,
            crate::kernel::state_root::SubsystemKind::Syscall,
            PhaseState::Phi1,
            PhaseState::Phi2,
            "identity_authority_state_checkpoint",
            None,
            &payload,
        );
        if let Ok(bytes) = self.identity.encode() {
            let _ = crate::kernel::syscall::persist_write_file(
                self,
                crate::kernel::identity::PHASEFS_STORE_PATH,
                &bytes,
            );
        }
    }

    /// Create a new non-Root principal. See
    /// `kernel::identity::Identity::create_principal`.
    #[cfg(all(feature = "identity", any(feature = "std", feature = "baremetal")))]
    pub fn identity_create_principal(
        &mut self,
        kind: crate::kernel::identity::PrincipalKind,
    ) -> Result<crate::kernel::identity::PrincipalId, crate::kernel::identity::PrincipalError> {
        let tick = self.statechain.tick();
        let result = self.identity.create_principal(kind, tick);
        if result.is_ok() {
            self.identity_checkpoint_statechain();
        }
        result
    }

    /// Disable a principal. See `kernel::identity::Identity::disable_principal`.
    #[cfg(all(feature = "identity", any(feature = "std", feature = "baremetal")))]
    pub fn identity_disable_principal(
        &mut self,
        id: crate::kernel::identity::PrincipalId,
    ) -> Result<(), crate::kernel::identity::PrincipalError> {
        let tick = self.statechain.tick();
        let result = self.identity.disable_principal(id, tick);
        if result.is_ok() {
            self.identity_checkpoint_statechain();
        }
        result
    }

    /// Enable a principal. See `kernel::identity::Identity::enable_principal`.
    #[cfg(all(feature = "identity", any(feature = "std", feature = "baremetal")))]
    pub fn identity_enable_principal(
        &mut self,
        id: crate::kernel::identity::PrincipalId,
    ) -> Result<(), crate::kernel::identity::PrincipalError> {
        let tick = self.statechain.tick();
        let result = self.identity.enable_principal(id, tick);
        if result.is_ok() {
            self.identity_checkpoint_statechain();
        }
        result
    }

    /// Install the pinned Root certificate. See
    /// `kernel::identity::Identity::install_root_certificate`.
    #[cfg(all(feature = "identity", any(feature = "std", feature = "baremetal")))]
    pub fn identity_install_root_certificate(
        &mut self,
        cert: crate::kernel::identity::UserCertificateV1,
        root_public_key: &[u8; 32],
    ) -> Result<crate::kernel::hash::StateHash, crate::kernel::identity::CertificateError> {
        let tick = self.statechain.tick();
        let result = self
            .identity
            .install_root_certificate(cert, root_public_key, tick);
        if result.is_ok() {
            self.identity_checkpoint_statechain();
        }
        result
    }

    /// Issue a new non-root certificate. See
    /// `kernel::identity::Identity::issue_certificate`.
    #[cfg(all(feature = "identity", any(feature = "std", feature = "baremetal")))]
    pub fn identity_issue_certificate(
        &mut self,
        cert: crate::kernel::identity::UserCertificateV1,
        now_generation: u64,
    ) -> Result<crate::kernel::hash::StateHash, crate::kernel::identity::CertificateError> {
        let tick = self.statechain.tick();
        let result = self.identity.issue_certificate(cert, now_generation, tick);
        if result.is_ok() {
            self.identity_checkpoint_statechain();
        }
        result
    }

    /// Revoke a certificate. See `kernel::identity::Identity::revoke_certificate`.
    #[cfg(all(feature = "identity", any(feature = "std", feature = "baremetal")))]
    pub fn identity_revoke_certificate(
        &mut self,
        hash: crate::kernel::hash::StateHash,
        principal_id: crate::kernel::identity::PrincipalId,
    ) -> Result<(), crate::kernel::identity::CertificateError> {
        let tick = self.statechain.tick();
        let result = self.identity.revoke_certificate(hash, principal_id, tick);
        if result.is_ok() {
            self.identity_checkpoint_statechain();
        }
        result
    }

    /// Provision the Root credential. See
    /// `kernel::identity::Identity::provision_root_credential`.
    #[cfg(all(feature = "identity", any(feature = "std", feature = "baremetal")))]
    pub fn identity_provision_root_credential(
        &mut self,
        verifier: crate::kernel::identity::PasswordVerifier,
    ) -> Result<(), crate::kernel::identity::CredentialError> {
        let tick = self.statechain.tick();
        let result = self.identity.provision_root_credential(verifier, tick);
        if result.is_ok() {
            self.identity_checkpoint_statechain();
        }
        result
    }

    /// Change the Root credential's password. See
    /// `kernel::identity::Identity::change_root_password`.
    #[cfg(all(feature = "identity", any(feature = "std", feature = "baremetal")))]
    pub fn identity_change_root_password(
        &mut self,
        policy: &crate::kernel::identity::PasswordPolicy,
        verifier: crate::kernel::identity::PasswordVerifier,
    ) -> Result<(), crate::kernel::identity::CredentialError> {
        let tick = self.statechain.tick();
        let result = self.identity.change_root_password(policy, verifier, tick);
        if result.is_ok() {
            self.identity_checkpoint_statechain();
        }
        result
    }

    /// Attempt Root authentication. See
    /// `kernel::identity::Identity::authenticate_root_credential`.
    #[cfg(all(feature = "identity", any(feature = "std", feature = "baremetal")))]
    pub fn identity_authenticate_root_credential(
        &mut self,
        policy: &crate::kernel::identity::PasswordPolicy,
        password: &[u8],
        now_chronos: u64,
    ) -> Result<bool, crate::kernel::identity::CredentialError> {
        let tick = self.statechain.tick();
        let before_sequence = self.identity.ledger().sequence();
        let result = self
            .identity
            .authenticate_root_credential(policy, password, now_chronos, tick);
        if self.identity.ledger().sequence() != before_sequence {
            self.identity_checkpoint_statechain();
        }
        result
    }

    /// Explicitly lock the Root credential. See
    /// `kernel::identity::Identity::lock_root_credential`.
    #[cfg(all(feature = "identity", any(feature = "std", feature = "baremetal")))]
    pub fn identity_lock_root_credential(&mut self) {
        let tick = self.statechain.tick();
        let before_sequence = self.identity.ledger().sequence();
        self.identity.lock_root_credential(tick);
        if self.identity.ledger().sequence() != before_sequence {
            self.identity_checkpoint_statechain();
        }
    }

    /// Unlock the Root credential. See
    /// `kernel::identity::Identity::unlock_root_credential`.
    #[cfg(all(feature = "identity", any(feature = "std", feature = "baremetal")))]
    pub fn identity_unlock_root_credential(
        &mut self,
    ) -> Result<(), crate::kernel::identity::CredentialError> {
        let tick = self.statechain.tick();
        let result = self.identity.unlock_root_credential(tick);
        if result.is_ok() {
            self.identity_checkpoint_statechain();
        }
        result
    }

    /// Require recovery for the Root credential. See
    /// `kernel::identity::Identity::require_root_recovery`.
    #[cfg(all(feature = "identity", any(feature = "std", feature = "baremetal")))]
    pub fn identity_require_root_recovery(&mut self) {
        let tick = self.statechain.tick();
        self.identity.require_root_recovery(tick);
        self.identity_checkpoint_statechain();
    }

    /// Write the live identity snapshot to `store` (Gate 6).
    #[cfg(all(feature = "identity", any(feature = "std", feature = "baremetal")))]
    pub fn identity_persist(
        &self,
        store: &mut dyn crate::kernel::identity::IdentityStore,
    ) -> Result<(), crate::kernel::identity::StoreError> {
        self.identity.persist_to(store)
    }

    /// Load a previously persisted identity snapshot. `StoreError::NotFound`
    /// means there is no file yet (first boot) — callers typically treat
    /// that as success-with-defaults.
    #[cfg(all(feature = "identity", any(feature = "std", feature = "baremetal")))]
    pub fn identity_restore(
        &mut self,
        store: &mut dyn crate::kernel::identity::IdentityStore,
    ) -> Result<(), crate::kernel::identity::StoreError> {
        self.identity.load_from(store)
    }

    /// Authenticate Root and open a local password session bound to this
    /// boot's `BootSessionRoot` (Gate 7).
    #[cfg(all(feature = "identity", any(feature = "std", feature = "baremetal")))]
    pub fn identity_login_root(
        &mut self,
        password: &[u8],
        caller: crate::kernel::event_log::ProcessId,
    ) -> Result<crate::kernel::identity::UserSession, crate::kernel::identity::AuthorityError> {
        let policy = *self.identity.password_policy();
        let now = self.clock.chronos_tick;
        match self.identity_authenticate_root_credential(&policy, password, now) {
            Ok(true) => {}
            _ => return Err(crate::kernel::identity::AuthorityError::AuthenticationFailed),
        }
        let tick = self.statechain.tick();
        let boot = self.authority.boot_session_root;
        let session = self
            .identity
            .open_session(
                crate::kernel::identity::ROOT_PRINCIPAL_ID,
                crate::kernel::identity::SessionOrigin::Local,
                crate::kernel::identity::AuthenticationMethod::Password,
                boot,
                tick,
                caller,
            )
            .map_err(crate::kernel::identity::AuthorityError::Session)?;
        self.identity_checkpoint_statechain();
        Ok(session)
    }

    #[cfg(all(feature = "identity", any(feature = "std", feature = "baremetal")))]
    pub fn identity_logout(
        &mut self,
        caller: crate::kernel::event_log::ProcessId,
    ) -> Result<(), crate::kernel::identity::SessionError> {
        let sid = self
            .identity
            .sessions()
            .context_of(caller)
            .ok_or(crate::kernel::identity::SessionError::NotFound)?;
        let tick = self.statechain.tick();
        self.identity.close_session(sid, tick)?;
        self.identity_checkpoint_statechain();
        Ok(())
    }

    #[cfg(all(feature = "identity", any(feature = "std", feature = "baremetal")))]
    pub fn identity_issue_mod_ticket(
        &mut self,
        session_id: crate::kernel::identity::SessionId,
        op: crate::kernel::identity::ModOperation,
        lease_ticks: u64,
    ) -> Result<crate::kernel::identity::ModTicket, crate::kernel::identity::AuthorityError> {
        let tick = self.statechain.tick();
        let boot = self.authority.boot_session_root;
        let ticket = self
            .identity
            .issue_mod_ticket(session_id, op, boot, tick, lease_ticks)?;
        self.identity_checkpoint_statechain();
        Ok(ticket)
    }

    #[cfg(all(feature = "identity", any(feature = "std", feature = "baremetal")))]
    pub fn identity_require(
        &self,
        ticket_id: u32,
        op: crate::kernel::identity::ModOperation,
    ) -> Result<(), crate::kernel::identity::AuthorityError> {
        self.identity.require(
            ticket_id,
            op,
            self.authority.boot_session_root,
            self.statechain.tick(),
        )
    }

    #[cfg(all(feature = "identity", any(feature = "std", feature = "baremetal")))]
    pub fn identity_revoke_ticket(
        &mut self,
        ticket_id: u32,
    ) -> Result<(), crate::kernel::identity::TicketError> {
        let tick = self.statechain.tick();
        let result = self.identity.revoke_ticket(ticket_id, tick);
        if result.is_ok() {
            self.identity_checkpoint_statechain();
        }
        result
    }

    #[cfg(all(feature = "phasesentinel", any(feature = "std", feature = "baremetal")))]
    fn sentinel_checkpoint_statechain(&mut self) {
        let seq = self.sentinel.evidence.sequence();
        let drops = self.sentinel.evidence.drop_count();
        let head = self.sentinel.evidence.head_hash();
        let mut payload = seq.to_le_bytes().to_vec();
        payload.extend_from_slice(&drops.to_le_bytes());
        payload.extend_from_slice(&head);
        let _ = self.statechain.record_transition(
            crate::kernel::state_proof::ObjectKind::Syscall,
            crate::kernel::sentinel::SENTINEL_EVIDENCE_OBJECT_ID,
            crate::kernel::state_root::SubsystemKind::Syscall,
            PhaseState::Phi1,
            PhaseState::Phi2,
            "sentinel_evidence_checkpoint",
            None,
            &payload,
        );
    }

    #[cfg(all(feature = "phasesentinel", any(feature = "std", feature = "baremetal")))]
    fn sentinel_sched_epoch(&self) -> Option<u64> {
        #[cfg(all(feature = "sched2", any(feature = "std", feature = "baremetal")))]
        {
            return Some(self.sched2.elapsed());
        }
        #[cfg(not(all(feature = "sched2", any(feature = "std", feature = "baremetal"))))]
        None
    }

    #[cfg(all(feature = "phasesentinel", any(feature = "std", feature = "baremetal")))]
    pub fn sentinel_authorize_launch(
        &mut self,
        pid: ProcessId,
        program: &[u8],
    ) -> Result<crate::kernel::hash::StateHash, crate::kernel::sentinel::SentinelError> {
        use crate::kernel::crypto::phase_hash::ph_content;
        use crate::kernel::sentinel::event::{EventKind, SentinelEvent};
        use crate::kernel::sentinel::launch;
        let content = ph_content(program);
        let now = self.statechain.tick();
        let manifest = self
            .sentinel
            .manifest_for(&content)
            .cloned()
            .ok_or(crate::kernel::sentinel::SentinelError::Unsigned)?;
        let result = launch::authorize_launch(&mut self.sentinel, pid, program, &manifest, now);
        let decision = match &result {
            Ok(_) => crate::kernel::sentinel::SentinelDecision::Allow,
            Err(crate::kernel::sentinel::SentinelError::Quarantined) => {
                crate::kernel::sentinel::SentinelDecision::Quarantine
            }
            Err(crate::kernel::sentinel::SentinelError::Isolated) => {
                crate::kernel::sentinel::SentinelDecision::Isolate
            }
            Err(_) => crate::kernel::sentinel::SentinelDecision::Refuse,
        };
        self.sentinel.record_decision(
            SentinelEvent {
                kind: EventKind::Launch,
                pid,
                nr: 0,
                from: None,
                to: None,
                content: Some(content),
            },
            decision,
        );
        self.sentinel_checkpoint_statechain();
        result
    }

    #[cfg(all(feature = "phasesentinel", any(feature = "std", feature = "baremetal")))]
    pub fn sentinel_observe_syscall(&mut self, pid: ProcessId, nr: u32) {
        use crate::kernel::sentinel::event::{EventKind, SentinelEvent};
        if self.sentinel_sched_epoch().is_none() {
            self.sentinel.bump_local_epoch();
        }
        let kind = match nr {
            crate::kernel::syscall::SPAWN => EventKind::Spawn,
            crate::kernel::syscall::EXIT => EventKind::Exit,
            crate::kernel::syscall::FS_OPEN
            | crate::kernel::syscall::FS_READ_FILE
            | crate::kernel::syscall::FS_WRITE_FILE
            | crate::kernel::syscall::FS_MKDIR
            | crate::kernel::syscall::FS_RM => EventKind::Fs,
            crate::kernel::syscall::IPC_CREATE_QUEUE
            | crate::kernel::syscall::IPC_SEND
            | crate::kernel::syscall::IPC_RECV
            | crate::kernel::syscall::IPC_DESTROY_QUEUE => EventKind::Ipc,
            _ => EventKind::Syscall,
        };
        self.sentinel.record_observe(SentinelEvent {
            kind,
            pid,
            nr,
            from: None,
            to: None,
            content: None,
        });
    }

    #[cfg(all(feature = "phasesentinel", any(feature = "std", feature = "baremetal")))]
    pub fn sentinel_observe_phasenet(&mut self, pid: ProcessId, verdict_u8: u8) {
        use crate::kernel::sentinel::event::{EventKind, SentinelEvent};
        self.sentinel.record_observe(SentinelEvent {
            kind: EventKind::PhaseNet,
            pid,
            nr: verdict_u8 as u32,
            from: None,
            to: None,
            content: None,
        });
    }

    #[cfg(all(feature = "phasesentinel", any(feature = "std", feature = "baremetal")))]
    pub fn sentinel_authorize_syscall(
        &self,
        pid: ProcessId,
    ) -> Result<(), crate::kernel::sentinel::SentinelError> {
        match self.sentinel.disposition(pid) {
            crate::kernel::sentinel::TaskDisposition::Isolated => {
                Err(crate::kernel::sentinel::SentinelError::Isolated)
            }
            crate::kernel::sentinel::TaskDisposition::Quarantined => {
                Err(crate::kernel::sentinel::SentinelError::Quarantined)
            }
            crate::kernel::sentinel::TaskDisposition::None => Ok(()),
        }
    }

    #[cfg(all(feature = "phasesentinel", any(feature = "std", feature = "baremetal")))]
    pub fn sentinel_quarantine_task(
        &mut self,
        pid: ProcessId,
    ) -> Result<(), crate::kernel::sentinel::SentinelError> {
        self.sentinel_contain_task(pid, crate::kernel::sentinel::TaskDisposition::Quarantined)
    }

    #[cfg(all(feature = "phasesentinel", any(feature = "std", feature = "baremetal")))]
    pub fn sentinel_isolate_task(
        &mut self,
        pid: ProcessId,
    ) -> Result<(), crate::kernel::sentinel::SentinelError> {
        self.sentinel_contain_task(pid, crate::kernel::sentinel::TaskDisposition::Isolated)
    }

    #[cfg(all(feature = "phasesentinel", any(feature = "std", feature = "baremetal")))]
    fn sentinel_contain_task(
        &mut self,
        pid: ProcessId,
        disp: crate::kernel::sentinel::TaskDisposition,
    ) -> Result<(), crate::kernel::sentinel::SentinelError> {
        use crate::kernel::sentinel::decision::SentinelDecision;
        use crate::kernel::sentinel::event::{EventKind, SentinelEvent};
        #[cfg(all(target_arch = "arm", feature = "mmu-app-isolation"))]
        {
            crate::arch::armv7::isolation::unmap_contained(pid);
        }
        let _ = self.kill_process(pid);
        #[cfg(all(feature = "captable", any(feature = "std", feature = "baremetal")))]
        {
            let _ = self.captable.close_all_for_subject(pid);
        }
        #[cfg(all(feature = "sched2", any(feature = "std", feature = "baremetal")))]
        {
            if disp == crate::kernel::sentinel::TaskDisposition::Isolated {
                if let Some(h) = self.sched2.handle_for_pid(pid) {
                    let _ = self.sched2.exit(h);
                }
            }
        }
        if let Some(session) = self.identity.sessions().session_for_process(pid) {
            self.identity.caps_mut().revoke_session(session.session_id);
        }
        let content = self
            .sentinel
            .cached_launch(pid)
            .map(|c| c.content_hash);
        self.sentinel.set_disposition(pid, disp);
        if let Some(c) = content {
            self.sentinel.block_content(c, disp);
        }
        self.sentinel.drop_cache(pid);
        let decision = match disp {
            crate::kernel::sentinel::TaskDisposition::Isolated => SentinelDecision::Isolate,
            _ => SentinelDecision::Quarantine,
        };
        self.sentinel.record_decision(
            SentinelEvent {
                kind: EventKind::Exit,
                pid,
                nr: 0,
                from: None,
                to: Some(PhaseState::Phi0),
                content,
            },
            decision,
        );
        self.sentinel_checkpoint_statechain();
        Ok(())
    }

    #[cfg(all(feature = "phasesentinel", any(feature = "std", feature = "baremetal")))]
    pub fn sentinel_recover_task(
        &mut self,
        pid: ProcessId,
        checkpoint: crate::kernel::sentinel::SentinelCheckpoint,
    ) -> Result<(), crate::kernel::sentinel::SentinelError> {
        let root_pk = self
            .sentinel
            .signers
            .root_pubkey
            .ok_or(crate::kernel::sentinel::SentinelError::NoAnchor)?;
        checkpoint.verify(&root_pk)?;
        let content = checkpoint.content_root;
        self.sentinel.push_checkpoint(checkpoint);
        self.sentinel.clear_content_block(&content);
        self.sentinel
            .set_disposition(pid, crate::kernel::sentinel::TaskDisposition::None);
        self.sentinel_checkpoint_statechain();
        Ok(())
    }

    #[allow(unused_variables)]
    fn with_process_transition<F>(
        &mut self,
        pid: ProcessId,
        op: &str,
        f: F,
    ) -> Result<bool, &'static str>
    where
        F: FnOnce(&mut Scheduler) -> Result<bool, &'static str>,
    {
        let old = self.get_process(pid)?.state;
        let ok = f(&mut self.scheduler)?;
        if ok {
            let new = self.get_process(pid)?.state;
            if old != new {
                // Gate-4: stash phase endpoints for the Kairos observe hook inside
                // record_kairos_on_kernel. Does not block, repair, or alter scheduling.
                #[cfg(feature = "phaseguard-research")]
                {
                    self.phaseguard_pending = Some((old, old, new));
                }
                self.trace.record(TraceRecord::PhaseChange {
                    pid,
                    from: old,
                    to: new,
                });
                stateclock::record_kairos_on_kernel(
                    self,
                    stateclock::KairosEventKind::PhaseChange,
                    format!("pid {pid} {old:?}->{new:?} ({op})"),
                );
            }
            #[cfg(any(feature = "std", feature = "baremetal"))]
            self.record_process(pid, old, new, op);
        }
        Ok(ok)
    }

    #[allow(unused_variables)]
    fn with_page_transition<F>(&mut self, page: PageId, op: &str, f: F) -> Result<(), MemoryError>
    where
        F: FnOnce(&mut Memory) -> Result<(), MemoryError>,
    {
        let old = *self
            .memory
            .tag_report()
            .get(&page)
            .unwrap_or(&PhaseState::Phi0);
        f(&mut self.memory)?;
        let new = *self
            .memory
            .tag_report()
            .get(&page)
            .unwrap_or(&PhaseState::Phi0);
        #[cfg(feature = "std")]
        self.record_memory(page, old, new, op);
        Ok(())
    }

    pub fn wake(&mut self, pid: ProcessId) -> Result<bool, &'static str> {
        self.with_process_transition(pid, "wake", |s| s.wake(pid))
    }

    pub fn schedule(&mut self, pid: ProcessId) -> Result<bool, &'static str> {
        self.with_process_transition(pid, "schedule", |s| s.schedule(pid))
    }

    pub fn preempt(&mut self, pid: ProcessId) -> Result<bool, &'static str> {
        self.with_process_transition(pid, "preempt", |s| s.preempt(pid))
    }

    pub fn syscall(&mut self, pid: ProcessId) -> Result<bool, &'static str> {
        self.with_process_transition(pid, "syscall", |s| s.syscall(pid))
    }

    pub fn kreturn(&mut self, pid: ProcessId) -> Result<bool, &'static str> {
        self.with_process_transition(pid, "kreturn", |s| s.kreturn(pid))
    }

    pub fn exit(&mut self, pid: ProcessId) -> Result<bool, &'static str> {
        self.with_process_transition(pid, "exit", |s| s.exit(pid))
    }

    pub fn get_process(&self, pid: ProcessId) -> Result<&Process, &'static str> {
        self.scheduler.get_process(pid)
    }

    pub fn alloc_page(&mut self) -> PageId {
        let page = self.memory.alloc_page();
        #[cfg(feature = "std")]
        self.record_memory(page, PhaseState::Phi0, PhaseState::Phi0, "alloc");
        #[cfg(all(feature = "pmm", any(feature = "std", feature = "baremetal")))]
        if let Some(addr) = self.physical_memory.alloc(1) {
            self.page_frames.insert(page, addr);
        }
        page
    }

    pub fn free_page(&mut self, page_id: PageId) -> Result<(), MemoryError> {
        let result = self.with_page_transition(page_id, "free", |m| m.free_page(page_id));
        #[cfg(all(feature = "pmm", any(feature = "std", feature = "baremetal")))]
        if result.is_ok() {
            if let Some(addr) = self.page_frames.remove(&page_id) {
                let _ = self.physical_memory.free(addr);
            }
        }
        result
    }

    /// Host/simulation region is NOT real physical DDR — see the explicit
    /// non-claim in `fpga/zynq/evidence/PMM_STATUS_*.md`. On the real board
    /// this uses `pmm::zynq7000_usable_region()` (usable DDR beyond the
    /// linker-reserved kernel image/stacks/heap).
    #[cfg(all(feature = "pmm", any(feature = "std", feature = "baremetal")))]
    fn new_physical_memory_manager() -> PhysicalMemoryManager {
        #[cfg(all(target_arch = "arm", feature = "zynq-a9"))]
        {
            let (base, frame_count) = crate::kernel::pmm::zynq7000_usable_region();
            PhysicalMemoryManager::new(base, frame_count)
        }
        #[cfg(not(all(target_arch = "arm", feature = "zynq-a9")))]
        {
            PhysicalMemoryManager::new(0x1000_0000, 4096)
        }
    }

    pub fn map_user(&mut self, page_id: PageId) -> Result<(), MemoryError> {
        self.with_page_transition(page_id, "map_user", |m| m.map_user(page_id))
    }

    pub fn map_kernel(&mut self, page_id: PageId) -> Result<(), MemoryError> {
        self.with_page_transition(page_id, "map_kernel", |m| m.map_kernel(page_id))
    }

    pub fn escalate(&mut self, page_id: PageId) -> Result<(), MemoryError> {
        self.with_page_transition(page_id, "escalate", |m| m.escalate(page_id))
    }

    pub fn lock_page(&mut self, page_id: PageId) -> Result<(), MemoryError> {
        self.with_page_transition(page_id, "lock_page", |m| m.lock_page(page_id))
    }

    pub fn unlock_page(&mut self, page_id: PageId) -> Result<(), MemoryError> {
        self.with_page_transition(page_id, "unlock_page", |m| m.unlock_page(page_id))
    }

    pub fn unmap_user(&mut self, page_id: PageId) -> Result<(), MemoryError> {
        self.with_page_transition(page_id, "unmap_user", |m| m.unmap_user(page_id))
    }

    pub fn unmap_kernel(&mut self, page_id: PageId) -> Result<(), MemoryError> {
        self.with_page_transition(page_id, "unmap_kernel", |m| m.unmap_kernel(page_id))
    }

    pub fn topology_report(&self) -> Vec<TopologyEvent> {
        self.scheduler.topology_report()
    }

    pub fn contention_check(
        &self,
        pid_a: ProcessId,
        pid_b: ProcessId,
    ) -> Result<PhaseState, &'static str> {
        self.scheduler.contention(pid_a, pid_b, &self.consensus)
    }

    pub fn tag_report(&self) -> HashMap<PageId, PhaseState> {
        self.memory.tag_report()
    }

    pub fn process_ids(&self) -> Vec<ProcessId> {
        self.scheduler.process_ids()
    }

    pub fn memory_arbitrate(
        &self,
        page_a: PageId,
        page_b: PageId,
    ) -> Result<PhaseState, MemoryError> {
        self.memory.arbitrate(page_a, page_b, &self.consensus)
    }

    pub fn attach_phase_accelerator(&mut self, accel: Box<dyn PhaseAccelerator>) {
        self.consensus.attach(accel);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KernelState {
    pub process_count: usize,
    pub page_count: usize,
    pub running_count: usize,
}
