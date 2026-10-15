//! Side-by-side scale benchmark: binary fast doubling vs KairOS phase doubling.

use kairos::kernel::phase_benchmark::scale_benchmark_compare;

fn median_f64(samples: &[f64]) -> f64 {
    let mut sorted = samples.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    }
}

fn throughput_speedup(single_ms: f64, workers: usize, batch_ms: f64) -> f64 {
    if batch_ms <= 0.0 {
        0.0
    } else {
        (workers as f64 * single_ms) / batch_ms
    }
}

#[test]
fn export_scale_compare_graph_data() {
    let target = 1_000_000u64;
    let max_cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let mut worker_counts = vec![1usize, 2, 4, 8];
    if max_cpus > 8 && !worker_counts.contains(&max_cpus) {
        worker_counts.push(max_cpus);
    }
    let reps = 5u32;

    println!("{{");
    println!("  \"target\": {target},");
    println!("  \"repetitions\": {reps},");
    println!("  \"profile\": \"release\",");
    println!("  \"logical_cpus\": {max_cpus},");
    println!("  \"points\": [");

    for (wi, workers) in worker_counts.iter().enumerate() {
        let mut binary_ms = Vec::new();
        let mut phase_ms = Vec::new();
        let mut binary_jps = Vec::new();
        let mut phase_jps = Vec::new();
        let mut binary_single = Vec::new();
        let mut phase_single = Vec::new();
        let mut match_ok = true;

        for _ in 0..reps {
            let report = scale_benchmark_compare(target, *workers).expect("scale compare");
            match_ok &= report.result_equality;
            binary_ms.push(report.binary_fast.total_ns as f64 / 1_000_000.0);
            phase_ms.push(report.kairos_phase.total_ns as f64 / 1_000_000.0);
            binary_jps.push(report.binary_fast.jobs_per_second);
            phase_jps.push(report.kairos_phase.jobs_per_second);
            binary_single.push(report.binary_fast.single_job_ns as f64 / 1_000_000.0);
            phase_single.push(report.kairos_phase.single_job_ns as f64 / 1_000_000.0);
        }

        let b_ms = median_f64(&binary_ms);
        let p_ms = median_f64(&phase_ms);
        let b_single = median_f64(&binary_single);
        let p_single = median_f64(&phase_single);
        let b_speedup = throughput_speedup(b_single, *workers, b_ms);
        let p_speedup = throughput_speedup(p_single, *workers, p_ms);

        let comma = if wi + 1 < worker_counts.len() {
            ","
        } else {
            ""
        };
        println!(
            "    {{\"workers\": {workers}, \"result_match\": {}, \"binary_ms\": {b_ms:.2}, \"phase_ms\": {p_ms:.2}, \"binary_single_ms\": {b_single:.2}, \"phase_single_ms\": {p_single:.2}, \"binary_jps\": {:.1}, \"phase_jps\": {:.1}, \"binary_speedup\": {b_speedup:.2}, \"phase_speedup\": {p_speedup:.2}, \"parallel_ratio\": {:.4} }}{comma}",
            if match_ok { "true" } else { "false" },
            median_f64(&binary_jps),
            median_f64(&phase_jps),
            if p_ms > 0.0 { b_ms / p_ms } else { 1.0 }
        );
    }

    println!("  ]");
    println!("}}");
}
