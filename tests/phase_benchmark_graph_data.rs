//! One-shot benchmark data collector for graph canvas (stdout JSON).

use kairos::kernel::phase_benchmark::engines::{
    fib_binary_fast_doubling, fib_binary_linear, fib_kairos_phase_doubling,
};
use kairos::kernel::phase_math::tables::PhaseMathTables;
use std::time::Instant;

fn median_ms(samples: &[u128]) -> f64 {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let mid = sorted.len() / 2;
    let median = if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2
    } else {
        sorted[mid]
    };
    median as f64 / 1_000_000.0
}

fn time_engine<F: Fn()>(f: F, reps: u32) -> f64 {
    let mut samples = Vec::new();
    for _ in 0..reps {
        let start = Instant::now();
        f();
        samples.push(start.elapsed().as_nanos());
    }
    median_ms(&samples)
}

#[test]
fn export_graph_data() {
    let tables = PhaseMathTables::build();
    let targets = [50u64, 100, 500, 1_000, 5_000, 10_000, 100_000, 1_000_000];
    let reps = 20u32;

    println!("{{");
    println!("  \"repetitions\": {reps},");
    println!("  \"machine\": \"hosted-debug\",");
    println!("  \"points\": [");

    for (i, n) in targets.iter().enumerate() {
        let linear = time_engine(
            || {
                let _ = fib_binary_linear(*n);
            },
            reps,
        );
        let binary = time_engine(
            || {
                let _ = fib_binary_fast_doubling(*n);
            },
            reps,
        );
        let phase = time_engine(
            || {
                let _ = fib_kairos_phase_doubling(*n, &tables);
            },
            reps,
        );

        let linear_out = fib_binary_linear(*n);
        let digits = linear_out.current.to_string().len();

        let comma = if i + 1 < targets.len() { "," } else { "" };
        println!(
            "    {{\"n\": {n}, \"digits\": {digits}, \"linear_ms\": {linear:.4}, \"binary_ms\": {binary:.4}, \"phase_ms\": {phase:.4}, \"speedup\": {speedup:.4} }}{comma}",
            speedup = binary / phase
        );
    }

    println!("  ]");
    println!("}}");
}
