//! PHI-STABILITY test runner.

use crate::core::gphase_core::PhaseState;
use crate::kernel::hash::StateHash;
use crate::kernel::phase_math::tables::legal_transition;
use crate::kernel::phi_stability::dual_clock::{chronos_elapsed, DualClockRecorder};
use crate::kernel::phi_stability::error::PhiStabilityError;
use crate::kernel::phi_stability::fibonacci::{fib_pair_with_steps, recurrence_steps};
use crate::kernel::phi_stability::metrics::format_run_result;
use crate::kernel::phi_stability::network::{
    apply_pending_fault, build_network_message, receive_and_verify, replay_last, send_message,
};
use crate::kernel::phi_stability::proof::{
    clock_root, collect_clock_hashes, collect_step_hashes, hash_fib_value, hash_pair_commitment,
    result_hash, state_event_hash, test_root, trajectory_root, verify_step_chain,
};
use crate::kernel::phi_stability::trajectory::{lifecycle_phase_for_step, TrajectoryBuilder};
#[cfg(feature = "std")]
use crate::kernel::phi_stability::types::PHI_STABILITY_STATECHAIN_OBJECT_ID;
use crate::kernel::phi_stability::types::{
    PhiKairosEventKind, PhiStabilityResult, PhiStateEvent, PhiTestId, PhiTestMode,
};
#[cfg(feature = "std")]
use crate::kernel::state_proof::ObjectKind;
#[cfg(feature = "std")]
use crate::kernel::state_root::SubsystemKind;
use crate::kernel::Kernel;
use crate::platform::{String, Vec};

struct RunContext {
    _test_id: PhiTestId,
    _target: u64,
    _mode: PhiTestMode,
    _dual_clock: bool,
}

pub fn run_local(
    kernel: &mut Kernel,
    target: u64,
    dual_clock: bool,
) -> Result<String, PhiStabilityError> {
    let result = execute_run(
        kernel,
        target,
        if dual_clock {
            PhiTestMode::DualClock
        } else {
            PhiTestMode::Local
        },
    )?;
    Ok(format_run_result(&result))
}

pub fn run_compare(kernel: &mut Kernel, target: u64) -> Result<String, PhiStabilityError> {
    let primary = execute_run(kernel, target, PhiTestMode::ScalarComparison)?;
    let scalar_ok = kernel.phase_math.verify_scalar_equivalence();
    let match_hash = primary.result_hash;
    Ok(format!(
        "scalar backend:      {}\n\
active backend:      {}\n\
result hash:         {}\n\
pair hash:           {}\n\
Kairos sequence:     {}\n\
backend equivalence: {}\n",
        if scalar_ok { "PASS" } else { "FAIL" },
        if scalar_ok { "PASS" } else { "FAIL" },
        crate::kernel::phi_stability::merkle::short_hash(&match_hash),
        crate::kernel::phi_stability::merkle::short_hash(&primary.pair_hash),
        if primary.proof_valid {
            "MATCH"
        } else {
            "MISMATCH"
        },
        if scalar_ok { "OK" } else { "FAIL" },
    ))
}

pub fn compute_result_hash_only(
    kernel: &mut Kernel,
    target: u64,
) -> Result<StateHash, PhiStabilityError> {
    let integer_mode = kernel.phi_stability.integer_mode;
    let (pair, _) = fib_pair_with_steps(target)?;
    Ok(result_hash(target, integer_mode, &pair.current))
}

pub fn network_send(
    kernel: &mut Kernel,
    peer: &str,
    target: u64,
) -> Result<String, PhiStabilityError> {
    let result = execute_run(kernel, target, PhiTestMode::NetworkSender)?;
    let fault = kernel.phi_stability.pending_fault.take();
    let mut message = build_network_message(
        kernel,
        result.test_id,
        result.target_index,
        result.integer_mode,
        result.result_hash,
        result.pair_hash,
        result.trajectory_root,
        result.clock_root,
        result.merkle_root,
        result.recurrence_steps,
        result.chronos_elapsed,
        result.kairos_events,
        peer,
    );
    apply_pending_fault(&mut message, fault)?;
    send_message(kernel, peer, &mut message)?;
    kernel.phi_stability.network_tests += 1;
    kernel.phi_stability.last_network_message = Some(message);
    Ok(format!(
        "PHI-STABILITY NETWORK TEST\n\n\
test id:               {}\n\
destination:           {peer}\n\
target:                F({target})\n\
algorithm:             fast doubling\n\
integer mode:          {}\n\
recurrence steps:      {}\n\n\
local result hash:     {}\n\
trajectory root:       {}\n\
clock root:            {}\n\
Kairos events:         {}\n\
Chronos elapsed:       {}\n\n\
PhaseLink frame:       built\n\
EtherType:             0x88B5\n\
sequence:              {}\n\
transmit:              OK\n",
        result.test_id.0,
        result.integer_mode.label(),
        result.recurrence_steps,
        crate::kernel::phi_stability::merkle::short_hash(&result.result_hash),
        crate::kernel::phi_stability::merkle::short_hash(&result.trajectory_root),
        crate::kernel::phi_stability::merkle::short_hash(&result.clock_root),
        result.kairos_events,
        result.chronos_elapsed,
        kernel.phaselink.sender.sequence_number,
    ))
}

pub fn network_receive(kernel: &mut Kernel) -> Result<String, PhiStabilityError> {
    let report = receive_and_verify(kernel)?;
    if report.contains("MATCH") && report.contains("ACCEPT") {
        kernel.phi_stability.remote_matches += 1;
    }
    if report.contains("replay detected") {
        kernel.phi_stability.replays_quarantined += 1;
    }
    Ok(report)
}

pub fn fault_replay(kernel: &mut Kernel) -> Result<String, PhiStabilityError> {
    replay_last(kernel)?;
    Ok("phaselink replay sent".into())
}

pub fn fault_inject(kernel: &mut Kernel, fault_label: &str) -> Result<String, PhiStabilityError> {
    use crate::kernel::phi_stability::fault::fault_label as fl;
    use crate::kernel::phi_stability::types::PhiFaultType;
    let fault = PhiFaultType::from_label(fault_label).ok_or(PhiStabilityError::InvalidTarget)?;
    kernel.phi_stability.pending_fault = Some(fault);
    kernel.phi_stability.faults_detected += 1;
    Ok(format!("phi-stability fault armed: {}", fl(fault)))
}

pub fn status(kernel: &Kernel) -> String {
    crate::kernel::phi_stability::metrics::format_status(
        kernel.phi_stability.tests_completed,
        kernel.phi_stability.tests_failed,
        kernel.phi_stability.faults_detected,
        kernel.phi_stability.faults_missed,
        kernel.phase_math.backend.label(),
        kernel.phase_math.workers.active_worker_count,
        kernel.phi_stability.integer_mode.label(),
        kernel.phi_stability.last_target,
        &crate::kernel::phi_stability::merkle::short_hash(&kernel.phi_stability.last_result_hash),
        &crate::kernel::phi_stability::merkle::short_hash(
            &kernel.phi_stability.last_trajectory_root,
        ),
        kernel.phi_stability.last_chronos_elapsed,
        kernel.phi_stability.last_kairos_events,
        kernel.phi_stability.invalid_transitions,
        kernel.phi_stability.network_tests,
        kernel.phi_stability.remote_matches,
        kernel.phi_stability.replays_quarantined,
        true,
    )
}

fn execute_run(
    kernel: &mut Kernel,
    target: u64,
    mode: PhiTestMode,
) -> Result<PhiStabilityResult, PhiStabilityError> {
    let test_id = kernel.phi_stability.allocate_test_id();
    let ctx = RunContext {
        _test_id: test_id,
        _target: target,
        _mode: mode,
        _dual_clock: matches!(mode, PhiTestMode::DualClock),
    };
    let integer_mode = kernel.phi_stability.integer_mode;
    let chronos_start = kernel.clock.snapshot().chronos_tick;
    let kairos_start = kernel.clock.snapshot().kairos_tick;
    let mut recorder = DualClockRecorder::new();
    let mut trajectory = TrajectoryBuilder::new();
    let mut clock_snapshots = Vec::new();
    let mut phase = PhaseState::Phi0;
    let mut phase_transitions = 0u64;
    let mut invalid_transitions = 0u64;
    // Continue the global state-event hash chain from the kernel's last event.
    // `verify_state_chain` validates the whole `state_events` log, so seeding a
    // second test from zero broke the chain at the test boundary and reported
    // `statechain: FAIL` on every run after the first.
    let mut previous_event_hash = kernel
        .phi_stability
        .state_events
        .last()
        .map(|e| e.event_hash)
        .unwrap_or([0u8; 32]);

    record_state_event(
        kernel,
        &mut previous_event_hash,
        "phi_test_created",
        test_id,
        target,
    )?;
    let snap = kernel.clock.snapshot();
    recorder.record(PhiKairosEventKind::TestCreated, &snap, Some(kernel));
    clock_snapshots.push((
        snap.chronos_tick,
        snap.kairos_tick,
        snap.sector_id as i8,
        crate::kernel::phi_stability::trajectory::epsilon_fixed(&snap),
    ));

    phase = step_phase(
        &mut phase,
        PhaseState::Phi1,
        &mut phase_transitions,
        &mut invalid_transitions,
    )?;
    record_state_event(
        kernel,
        &mut previous_event_hash,
        "phi_operands_ready",
        test_id,
        target,
    )?;
    let snap = kernel.clock.snapshot();
    recorder.record(PhiKairosEventKind::OperandsInitialized, &snap, Some(kernel));

    let (pair, steps) = fib_pair_with_steps(target)?;
    let f_n_hash = hash_fib_value(&pair.current);
    let f_n1_hash = hash_fib_value(&pair.next);
    let bit_len = recurrence_steps(target);

    phase = step_phase(
        &mut phase,
        PhaseState::Phi2,
        &mut phase_transitions,
        &mut invalid_transitions,
    )?;
    let mut prev_step_hash = [0u8; 32];
    for step in 0..=bit_len {
        let input_bit = if step == 0 {
            false
        } else {
            (target >> (bit_len - step)) & 1 == 1
        };
        let (prev_p, next_p) = lifecycle_phase_for_step(step, bit_len);
        let snap = kernel.clock.snapshot();
        recorder.record(
            PhiKairosEventKind::DoublingStepCompleted,
            &snap,
            Some(kernel),
        );
        clock_snapshots.push((
            snap.chronos_tick,
            snap.kairos_tick,
            snap.sector_id as i8,
            crate::kernel::phi_stability::trajectory::epsilon_fixed(&snap),
        ));
        prev_step_hash = trajectory.record_step(
            test_id,
            step,
            input_bit,
            prev_p,
            next_p,
            &snap,
            &f_n_hash,
            &f_n1_hash,
            prev_step_hash,
        )?;
        record_state_event(
            kernel,
            &mut previous_event_hash,
            "phi_step_completed",
            test_id,
            target,
        )?;
    }

    record_state_event(
        kernel,
        &mut previous_event_hash,
        "phi_calculation_completed",
        test_id,
        target,
    )?;
    let snap = kernel.clock.snapshot();
    recorder.record(
        PhiKairosEventKind::CalculationCompleted,
        &snap,
        Some(kernel),
    );

    let res_hash = result_hash(target, integer_mode, &pair.current);
    let pair_h = hash_pair_commitment(&f_n_hash, &f_n1_hash);
    let step_hashes = collect_step_hashes(&trajectory.steps);
    let traj_root = trajectory_root(&step_hashes);
    let clock_hashes = collect_clock_hashes(&clock_snapshots);
    let clk_root = clock_root(&clock_hashes);
    let proof_ok = verify_step_chain(&trajectory.steps);
    recorder.record(PhiKairosEventKind::ResultVerified, &snap, Some(kernel));
    record_state_event(
        kernel,
        &mut previous_event_hash,
        "phi_result_verified",
        test_id,
        target,
    )?;

    if phase != PhaseState::Phi2 {
        phase = step_phase(
            &mut phase,
            PhaseState::Phi2,
            &mut phase_transitions,
            &mut invalid_transitions,
        )?;
    }
    phase = step_phase(
        &mut phase,
        PhaseState::Phi3,
        &mut phase_transitions,
        &mut invalid_transitions,
    )?;
    recorder.record(PhiKairosEventKind::TrajectorySealed, &snap, Some(kernel));
    record_state_event(
        kernel,
        &mut previous_event_hash,
        "phi_trajectory_sealed",
        test_id,
        target,
    )?;

    let equation_root = kernel.phaselink.equation_root;
    let boot_session_root = kernel.phaselink.boot_session_root;
    let merkle = test_root(
        &equation_root,
        &boot_session_root,
        test_id,
        target,
        &res_hash,
        &pair_h,
        &traj_root,
        &clk_root,
        phase_transitions,
        recorder.events.len() as u64,
    );
    recorder.record(PhiKairosEventKind::MerkleCommitted, &snap, Some(kernel));
    record_state_event(
        kernel,
        &mut previous_event_hash,
        "phi_merkle_committed",
        test_id,
        target,
    )?;

    let chronos_finish = kernel.clock.snapshot().chronos_tick;
    let kairos_finish = kernel.clock.snapshot().kairos_tick;
    // Record the terminal TestCompleted event *before* validating the dual-clock
    // policy: `expected_calculation_count` counts the complete event log
    // (8 + recurrence_steps, including TestCompleted). Validating first left the
    // log one event short, so `verify_policy` always returned false and
    // `phase trajectory` reported FAIL despite a correct result.
    let finish_snap = kernel.clock.snapshot();
    recorder.record(
        PhiKairosEventKind::TestCompleted,
        &finish_snap,
        Some(kernel),
    );
    let kairos_policy_ok = recorder.verify_policy(steps);
    let preview = if pair.current.digit_count() <= 32 {
        Some(pair.current.to_decimal_string())
    } else {
        None
    };

    let result = PhiStabilityResult {
        test_id,
        target_index: target,
        mode,
        integer_mode,
        recurrence_steps: steps,
        digit_count: Some(pair.current.digit_count()),
        result_preview: preview,
        result_hash: res_hash,
        pair_hash: pair_h,
        trajectory_root: traj_root,
        clock_root: clk_root,
        merkle_root: merkle,
        test_root: merkle,
        chronos_start,
        chronos_finish,
        chronos_elapsed: chronos_elapsed(chronos_start, chronos_finish),
        kairos_start,
        kairos_finish,
        kairos_events: recorder.events.len() as u64,
        phase_transitions,
        invalid_transitions,
        scalar_match: Some(kernel.phase_math.verify_scalar_equivalence()),
        remote_match: None,
        proof_valid: proof_ok && kairos_policy_ok,
        chain_valid: verify_state_chain(kernel),
    };

    kernel.phi_stability.tests_completed += 1;
    kernel.phi_stability.last_target = target;
    kernel.phi_stability.last_result_hash = res_hash;
    kernel.phi_stability.last_trajectory_root = traj_root;
    kernel.phi_stability.last_chronos_elapsed = result.chronos_elapsed;
    kernel.phi_stability.last_kairos_events = result.kairos_events;
    kernel.phi_stability.invalid_transitions += invalid_transitions;
    kernel.phi_stability.current_phase = phase;
    // TestCompleted is recorded earlier (before the dual-clock policy check).
    kernel.phi_stability.kairos_log = recorder.events;
    kernel.phi_stability.push_history(result.clone());
    record_state_event(
        kernel,
        &mut previous_event_hash,
        "phi_test_completed",
        test_id,
        target,
    )?;
    let _ = ctx;
    Ok(result)
}

fn step_phase(
    current: &mut PhaseState,
    target: PhaseState,
    transitions: &mut u64,
    invalid: &mut u64,
) -> Result<PhaseState, PhiStabilityError> {
    if !legal_transition(*current, target) && *current != target {
        *invalid += 1;
        return Err(PhiStabilityError::InvalidPhaseTransition);
    }
    if *current != target {
        *transitions += 1;
    }
    *current = target;
    Ok(target)
}

fn record_state_event(
    kernel: &mut Kernel,
    previous_hash: &mut StateHash,
    operation: &str,
    test_id: PhiTestId,
    target: u64,
) -> Result<(), PhiStabilityError> {
    let snap = kernel.clock.snapshot();
    let event_hash = state_event_hash(
        operation,
        test_id,
        target,
        snap.chronos_tick,
        snap.kairos_tick,
        previous_hash,
    );
    let event = PhiStateEvent {
        operation: operation.into(),
        test_id,
        target_index: target,
        chronos_tick: snap.chronos_tick,
        kairos_tick: snap.kairos_tick,
        event_hash,
        previous_event_hash: *previous_hash,
    };
    kernel.phi_stability.state_events.push(event);
    *previous_hash = event_hash;

    #[cfg(feature = "std")]
    {
        let _ = kernel.statechain.record_transition(
            ObjectKind::Driver,
            PHI_STABILITY_STATECHAIN_OBJECT_ID,
            SubsystemKind::Driver,
            PhaseState::Phi1,
            PhaseState::Phi2,
            operation,
            None,
            operation.as_bytes(),
        );
    }
    Ok(())
}

fn verify_state_chain(kernel: &Kernel) -> bool {
    let mut prev = [0u8; 32];
    for event in &kernel.phi_stability.state_events {
        if event.previous_event_hash != prev {
            return false;
        }
        prev = event.event_hash;
    }
    true
}

#[cfg(test)]
mod tests {
    use crate::kernel::Kernel;

    /// Regression: `phase trajectory` must report OK for a correct run.
    /// The dual-clock policy count (`8 + recurrence_steps`) includes the
    /// terminal TestCompleted event, which must be recorded before validation.
    #[test]
    fn run_local_reports_phase_trajectory_ok() {
        let mut kernel = Kernel::new();
        let out = super::run_local(&mut kernel, 50, false).expect("run_local");
        assert!(
            out.contains("phase trajectory:    OK"),
            "phase trajectory not OK:\n{out}"
        );
        assert!(
            !out.contains("phase trajectory:    FAIL"),
            "phase trajectory regressed to FAIL:\n{out}"
        );
        assert!(
            out.contains("result:              12586269025"),
            "F(50) result wrong:\n{out}"
        );
    }

    /// Regression: the global state-event chain must stay valid across multiple
    /// runs, so `statechain` stays OK on the second and later tests.
    #[test]
    fn statechain_stays_ok_across_multiple_runs() {
        let mut kernel = Kernel::new();
        let _ = super::run_local(&mut kernel, 50, false).expect("first run");
        let second = super::run_local(&mut kernel, 20, true).expect("second run");
        assert!(
            second.contains("statechain:          OK"),
            "statechain regressed to FAIL on the second run:\n{second}"
        );
        assert!(
            second.contains("phase trajectory:    OK"),
            "phase trajectory not OK on the second run:\n{second}"
        );
    }
}
