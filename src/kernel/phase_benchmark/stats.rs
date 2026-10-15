//! Timing statistics for phase benchmark.

use crate::kernel::phase_benchmark::error::PhaseBenchmarkError;
use crate::kernel::phase_benchmark::types::TimingStats;

pub fn summarize(samples_ns: &[u128]) -> Result<TimingStats, PhaseBenchmarkError> {
    if samples_ns.is_empty() {
        return Err(PhaseBenchmarkError::InsufficientRuns);
    }
    let mut sorted = samples_ns.to_vec();
    sorted.sort_unstable();
    let min_ns = *sorted.first().unwrap();
    let max_ns = *sorted.last().unwrap();
    if min_ns == 0 {
        return Err(PhaseBenchmarkError::ZeroDuration);
    }
    let median_ns = median(&sorted);
    let mean = sorted.iter().map(|v| *v as f64).sum::<f64>() / sorted.len() as f64;
    let variance = sorted
        .iter()
        .map(|v| {
            let d = *v as f64 - mean;
            d * d
        })
        .sum::<f64>()
        / sorted.len() as f64;
    let stddev_ns = variance.sqrt();
    let cv_percent = if mean > 0.0 {
        (stddev_ns / mean) * 100.0
    } else {
        0.0
    };
    Ok(TimingStats {
        samples_ns: sorted,
        median_ns,
        min_ns,
        max_ns,
        stddev_ns,
        cv_percent,
    })
}

fn median(sorted: &[u128]) -> u128 {
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2
    } else {
        sorted[mid]
    }
}

pub fn ns_to_ms(ns: u128) -> f64 {
    ns as f64 / 1_000_000.0
}

pub fn speedup_verdict(binary_median_ns: u128, phase_median_ns: u128) -> (&'static str, f64, f64) {
    let speedup = binary_median_ns as f64 / phase_median_ns as f64;
    let reduction = if binary_median_ns > 0 {
        ((binary_median_ns as f64 - phase_median_ns as f64) / binary_median_ns as f64) * 100.0
    } else {
        0.0
    };
    let diff_pct = ((phase_median_ns as f64 - binary_median_ns as f64).abs()
        / binary_median_ns as f64)
        * 100.0;
    let label = if diff_pct < 3.0 {
        "approximately equal"
    } else if speedup > 1.03 {
        if reduction > 25.0 {
            "KairOS strong difference (faster)"
        } else if reduction > 10.0 {
            "KairOS meaningful difference (faster)"
        } else {
            "KairOS small difference (faster)"
        }
    } else if speedup < 0.97 {
        "KairOS slower"
    } else {
        "approximately equal"
    };
    (label, speedup, reduction)
}

pub fn allocation_reduction(binary: u64, phase: u64) -> f64 {
    if binary == 0 {
        0.0
    } else {
        ((binary as f64 - phase as f64) / binary as f64) * 100.0
    }
}
