//! Phase-state benchmark integration tests.

use kairos::kernel::phase_benchmark::compare_inner;
use kairos::kernel::phase_benchmark::engines::{
    fib_binary_fast_doubling, fib_binary_linear, fib_kairos_phase_doubling,
};
use kairos::kernel::phase_benchmark::stats::summarize;
use kairos::kernel::phase_math::tables::PhaseMathTables;
use kairos::Kernel;

fn tables() -> PhaseMathTables {
    PhaseMathTables::build()
}

#[test]
fn fibonacci_correctness_biguint() {
    let linear = fib_binary_linear(50);
    let binary = fib_binary_fast_doubling(50);
    let phase = fib_kairos_phase_doubling(50, &tables());
    assert_eq!(linear.current, binary.current);
    assert_eq!(binary.current, phase.current);
    assert_eq!(linear.current.to_string(), "12586269025");
    for n in [0u64, 1, 2, 10, 50] {
        let b = fib_binary_fast_doubling(n);
        let p = fib_kairos_phase_doubling(n, &tables());
        assert_eq!(b.current, p.current);
        assert_eq!(b.next, p.next);
    }
}

#[test]
fn phase_engine_uses_tables() {
    let phase = fib_kairos_phase_doubling(100, &tables());
    assert!(phase.counters.phase_lookups > 0);
    assert!(phase.counters.transition_lookups > 0);
    assert!(phase.counters.compose_lookups > 0);
    assert!(phase.counters.topology_lookups > 0);
    assert!(phase.counters.chi_lookups > 0);
    assert_eq!(phase.counters.dynamic_branches, 0);
}

#[test]
fn compare_smoke() {
    let mut kernel = Kernel::new();
    let report = compare_inner(&mut kernel, 100, 20, false, false).unwrap();
    assert!(report.result_equality);
    assert_eq!(report.repetitions, 20);
    assert!(report.binary_fast.compute_stats.median_ns > 0);
    assert!(report.kairos_phase.compute_stats.median_ns > 0);
}

#[test]
fn compare_with_pipeline_smoke() {
    let mut kernel = Kernel::new();
    let report = compare_inner(&mut kernel, 50, 20, false, true).unwrap();
    assert!(report.full_pipeline.is_some());
}

#[test]
fn median_stats() {
    let stats = summarize(&[100, 200, 300, 400, 500]).unwrap();
    assert_eq!(stats.median_ns, 300);
    assert_eq!(stats.min_ns, 100);
    assert_eq!(stats.max_ns, 500);
}

#[test]
fn status_smoke() {
    let kernel = Kernel::new();
    let status = kairos::kernel::phase_benchmark::status(&kernel);
    assert!(status.contains("PHASE-STATE BENCHMARK"));
    assert!(status.contains("BigUint"));
}

#[test]
fn shell_compare_smoke() {
    let mut kernel = Kernel::new();
    let report = kairos::kernel::phase_benchmark::run_compare(&mut kernel, 50).unwrap();
    assert!(report.contains("KAIROS PHASE-STATE COMPUTATION BENCHMARK"));
    assert!(report.contains("MATCH"));
}
