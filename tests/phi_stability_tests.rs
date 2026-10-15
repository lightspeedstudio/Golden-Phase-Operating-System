//! PHI-STABILITY integration tests.

use kairos::kernel::phi_stability::fibonacci::{
    fib_linear_reference_u64, fib_u1024, recurrence_steps,
};
use kairos::kernel::phi_stability::fixed::{max_exact_target, U1024};
use kairos::kernel::phi_stability::proof::result_hash;
use kairos::kernel::phi_stability::types::PhiIntegerMode;
use kairos::Kernel;

#[test]
fn fibonacci_correctness() {
    assert_eq!(fib_u1024(0).unwrap().to_decimal_string(), "0");
    assert_eq!(fib_u1024(1).unwrap().to_decimal_string(), "1");
    assert_eq!(fib_u1024(2).unwrap().to_decimal_string(), "1");
    assert_eq!(fib_u1024(10).unwrap().to_decimal_string(), "55");
    assert_eq!(fib_u1024(50).unwrap().to_decimal_string(), "12586269025");
    for n in 0..=93 {
        assert_eq!(
            fib_u1024(n).unwrap().to_decimal_string(),
            fib_linear_reference_u64(n).unwrap().to_string()
        );
    }
}

#[test]
fn recurrence_step_count() {
    assert_eq!(recurrence_steps(0), 0);
    assert_eq!(recurrence_steps(100), 7);
}

#[test]
fn fixed_width_overflow() {
    let max = max_exact_target();
    assert!(fib_u1024(max).is_ok());
    assert!(fib_u1024(max.saturating_add(1)).is_err());
}

#[test]
fn result_hash_mode_tagged() {
    let value = U1024::from_u64(55);
    let exact = result_hash(10, PhiIntegerMode::ExactHosted, &value);
    let fixed = result_hash(
        10,
        PhiIntegerMode::FixedWidthBaremetal { bits: 1024 },
        &value,
    );
    assert_ne!(exact, fixed);
}

#[test]
fn local_run_smoke() {
    let mut kernel = Kernel::new();
    let report = kairos::kernel::phi_stability::run_local(&mut kernel, 50, false).unwrap();
    assert!(report.contains("PHI-STABILITY"));
    assert!(report.contains("12586269025"));
    assert!(report.contains("phase trajectory:"));
}

#[test]
fn dual_clock_run_smoke() {
    let mut kernel = Kernel::new();
    let report = kairos::kernel::phi_stability::run_local(&mut kernel, 100, true).unwrap();
    assert!(report.contains("PHI-STABILITY"));
    assert!(report.contains("Kairos events:"));
}

#[test]
fn compare_backend_smoke() {
    let mut kernel = Kernel::new();
    let report = kairos::kernel::phi_stability::run_compare(&mut kernel, 100).unwrap();
    assert!(report.contains("backend equivalence: OK"));
}

#[test]
fn state_event_chain_links() {
    let mut kernel = Kernel::new();
    let _ = kairos::kernel::phi_stability::run_local(&mut kernel, 20, false).unwrap();
    let mut prev = [0u8; 32];
    for event in &kernel.phi_stability.state_events {
        assert_eq!(event.previous_event_hash, prev);
        prev = event.event_hash;
    }
}
