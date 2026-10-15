use kairos::core::gphase_core::PhaseState;
use kairos::kernel::phase_math::batch::{BatchLane, PhaseBatchInput, PhaseOp, TopologyOp};
use kairos::kernel::phase_math::reference::verify_tables;
use kairos::kernel::phase_math::tables::{
    legal_transition, pair_index, triple_index, PhaseMathTables,
};
use kairos::userland::boot;
use kairos::userland::Hosted;
use kairos::Runtime;

fn booted() -> Hosted {
    let mut hosted = Hosted::new();
    boot(&mut hosted);
    hosted
}

#[test]
fn phase_math_tables_match_core() {
    let tables = PhaseMathTables::build();
    assert!(verify_tables(&tables));
    assert_eq!(
        tables.compose[pair_index(PhaseState::Phi1, PhaseState::Phi3)],
        kairos::core::gphase_core::compose(PhaseState::Phi1, PhaseState::Phi3).as_u8()
    );
    assert_eq!(
        tables.topology[triple_index(PhaseState::Phi1, PhaseState::Phi2, PhaseState::Phi3)],
        match kairos::core::gphase_core::topology_status(
            PhaseState::Phi1,
            PhaseState::Phi2,
            PhaseState::Phi3
        )
        .kind
        {
            kairos::core::gphase_core::TopologyKind::Same => 0,
            kairos::core::gphase_core::TopologyKind::Reflection => 1,
        }
    );
}

#[test]
fn legal_transition_table_is_stable() {
    assert!(legal_transition(PhaseState::Phi0, PhaseState::Phi1));
    assert!(legal_transition(PhaseState::Phi2, PhaseState::Phi3));
    assert!(!legal_transition(PhaseState::Phi0, PhaseState::Phi3));
}

#[test]
fn phase_math_batch_runs() {
    let mut hosted = booted();
    let batch = PhaseBatchInput {
        lane: Some(BatchLane::Immediate),
        phase_ops: vec![
            PhaseOp {
                left: PhaseState::Phi0,
                right: PhaseState::Phi1,
            },
            PhaseOp {
                left: PhaseState::Phi2,
                right: PhaseState::Phi3,
            },
        ],
        topology_ops: vec![TopologyOp {
            a: PhaseState::Phi1,
            b: PhaseState::Phi2,
            c: PhaseState::Phi3,
        }],
    };
    let out = hosted.kernel.phase_math.process_batch(&batch);
    assert_eq!(out.phase_results.len(), 2);
    assert_eq!(out.topology_results.len(), 1);
}

#[test]
fn process_transition_populates_queues() {
    let mut hosted = booted();
    let pid = hosted.init_pid.unwrap();
    hosted.kernel.syscall(pid).unwrap();
    assert!(!hosted.kernel.phase_math.transition_queue.is_empty());
    assert!(!hosted.kernel.phase_math.proof_pipeline.queue.is_empty());
    assert!(!hosted.kernel.phase_math.merkle_pipeline.queue.is_empty());
}

#[test]
fn phaseperf_shell_transcript() {
    let mut rt = Runtime::with_input([
        "phaseperf status",
        "phaseperf backend scalar",
        "phaseperf queues",
        "phaseperf benchmark",
        "phaseperf verify",
        "exit",
    ]);
    rt.boot().unwrap();
    let pid = rt.hosted.init_pid.unwrap();
    rt.hosted.kernel.syscall(pid).unwrap();
    rt.hosted.kernel.kreturn(pid).unwrap();
    rt.hosted.kernel.schedule(pid).unwrap();
    rt.run().unwrap();
    let out = rt.output_lines().join("\n");
    assert!(out.contains("phase math: enabled"));
    assert!(out.contains("active backend: scalar"));
    assert!(out.contains("benchmark: OK"));
    assert!(out.contains("scalar equivalence: OK"));
}
