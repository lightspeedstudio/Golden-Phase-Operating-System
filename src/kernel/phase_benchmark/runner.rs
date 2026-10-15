//! Phase-state benchmark runner.

use crate::kernel::hash::StateHash;
use crate::kernel::phase_benchmark::bigint::{self, BigUint};
use crate::kernel::phase_benchmark::engines;
use crate::kernel::phase_benchmark::error::PhaseBenchmarkError;
use crate::kernel::phase_benchmark::pipeline;
use crate::kernel::phase_benchmark::report::{
    format_compare, format_dual_clock, format_network_compare, format_scale_compare, format_status,
    format_sweep,
};
use crate::kernel::phase_benchmark::stats::{allocation_reduction, speedup_verdict, summarize};
use crate::kernel::phase_benchmark::types::{
    CompareReport, CompareVerdict, EngineCounters, EngineKind, EngineRunResult, ScaleCompareReport,
    ScaleReport, MIN_REPETITIONS,
};
use crate::kernel::Kernel;
use crate::platform::{String, Vec};

struct TimedOutput {
    output: engines::EngineOutput,
    compute_ns: u128,
}

pub fn status(kernel: &Kernel) -> String {
    format_status(
        kernel.phase_benchmark.runs_completed,
        kernel.phase_benchmark.last_target,
        &kernel.phase_benchmark.last_result_hash,
        kernel.phase_benchmark.default_repetitions,
        kernel.phase_math.workers.active_worker_count,
    )
}

pub fn run_single(kernel: &mut Kernel, target: u64) -> Result<String, PhaseBenchmarkError> {
    let report = compare_inner(kernel, target, 1, false, false)?;
    kernel.phase_benchmark.runs_completed += 1;
    kernel.phase_benchmark.last_target = target;
    kernel.phase_benchmark.last_result_hash = report.result_hash;
    Ok(format_compare(&report))
}

pub fn run_compare(kernel: &mut Kernel, target: u64) -> Result<String, PhaseBenchmarkError> {
    let reps = kernel
        .phase_benchmark
        .default_repetitions
        .max(MIN_REPETITIONS);
    let report = compare_inner(kernel, target, reps, false, true)?;
    store_report(kernel, &report);
    Ok(format_compare(&report))
}

pub fn run_repeat(
    kernel: &mut Kernel,
    target: u64,
    count: u32,
) -> Result<String, PhaseBenchmarkError> {
    let reps = count.max(MIN_REPETITIONS);
    let report = compare_inner(kernel, target, reps, true, true)?;
    store_report(kernel, &report);
    Ok(format_compare(&report))
}

pub fn run_sweep(kernel: &mut Kernel, start: u64, end: u64) -> Result<String, PhaseBenchmarkError> {
    if start > end {
        return Err(PhaseBenchmarkError::InvalidTarget);
    }
    let mut reports = Vec::new();
    let mut n = start;
    while n <= end {
        let report = compare_inner(kernel, n, MIN_REPETITIONS, true, false)?;
        reports.push(report);
        if n == end {
            break;
        }
        n = n.saturating_mul(10).max(n + 1);
        if n > end {
            n = end;
        }
    }
    if let Some(last) = reports.last() {
        store_report(kernel, last);
    }
    Ok(format_sweep(&reports))
}

pub fn run_scale(
    kernel: &mut Kernel,
    target: u64,
    workers: usize,
) -> Result<String, PhaseBenchmarkError> {
    let report = scale_benchmark_compare(target, workers)?;
    kernel.phase_benchmark.runs_completed += 1;
    Ok(format_scale_compare(&report))
}

pub fn scale_benchmark(target: u64, workers: usize) -> Result<ScaleReport, PhaseBenchmarkError> {
    scale_workers_engine(target, workers, EngineKind::KairosPhaseDoubling)
}

pub fn scale_benchmark_compare(
    target: u64,
    workers: usize,
) -> Result<ScaleCompareReport, PhaseBenchmarkError> {
    let binary_fast = scale_workers_engine(target, workers, EngineKind::BinaryFastDoubling)?;
    let kairos_phase = scale_workers_engine(target, workers, EngineKind::KairosPhaseDoubling)?;
    let binary_check = engines::fib_binary_fast_doubling(target);
    let phase_check = engines::fib_kairos_phase_doubling(
        target,
        &crate::kernel::phase_math::tables::PhaseMathTables::build(),
    );
    Ok(ScaleCompareReport {
        target,
        worker_count: workers,
        result_equality: binary_check.current == phase_check.current,
        binary_fast,
        kairos_phase,
    })
}

pub fn run_dual_clock(kernel: &mut Kernel, target: u64) -> Result<String, PhaseBenchmarkError> {
    let reps = kernel
        .phase_benchmark
        .default_repetitions
        .max(MIN_REPETITIONS);
    let report = compare_inner(kernel, target, reps, true, true)?;
    let pipeline = pipeline::run_full_pipeline(kernel, target);
    store_report(kernel, &report);
    Ok(format_dual_clock(
        &report,
        report.kairos_phase.compute_stats.median_ns,
        pipeline.kairos_events,
    ))
}

pub fn run_network_compare(
    kernel: &mut Kernel,
    peer: &str,
    target: u64,
) -> Result<String, PhaseBenchmarkError> {
    let reps = kernel
        .phase_benchmark
        .default_repetitions
        .max(MIN_REPETITIONS);
    let report = compare_inner(kernel, target, reps, true, true)?;
    store_report(kernel, &report);
    Ok(format_network_compare(peer, &report, None))
}

pub fn last_report(kernel: &Kernel) -> Result<String, PhaseBenchmarkError> {
    match &kernel.phase_benchmark.last_report {
        Some(report) => Ok(format_compare(report)),
        None => Err(PhaseBenchmarkError::InsufficientRuns),
    }
}

pub fn compare_inner(
    kernel: &mut Kernel,
    target: u64,
    repetitions: u32,
    include_linear: bool,
    include_full_pipeline: bool,
) -> Result<CompareReport, PhaseBenchmarkError> {
    if repetitions == 0 {
        return Err(PhaseBenchmarkError::InsufficientRuns);
    }

    warmup(kernel, target);

    let tables = kernel.phase_math.tables.clone();
    let mut binary_samples = Vec::new();
    let mut phase_samples = Vec::new();
    let mut binary_counters = EngineCounters::default();
    let mut phase_counters = EngineCounters::default();
    let mut binary_result: Option<BigUint> = None;
    let mut phase_result: Option<BigUint> = None;

    for run in 0..repetitions {
        let binary_first = run % 2 == 0;
        if binary_first {
            let timed = time_binary_fast(target);
            binary_samples.push(timed.compute_ns);
            merge_counters(&mut binary_counters, &timed.output.counters);
            binary_result = Some(timed.output.current.clone());
            let timed = time_kairos_phase(target, &tables);
            phase_samples.push(timed.compute_ns);
            merge_counters(&mut phase_counters, &timed.output.counters);
            phase_result = Some(timed.output.current.clone());
        } else {
            let timed = time_kairos_phase(target, &tables);
            phase_samples.push(timed.compute_ns);
            merge_counters(&mut phase_counters, &timed.output.counters);
            phase_result = Some(timed.output.current.clone());
            let timed = time_binary_fast(target);
            binary_samples.push(timed.compute_ns);
            merge_counters(&mut binary_counters, &timed.output.counters);
            binary_result = Some(timed.output.current.clone());
        }
    }

    let binary_value = binary_result.ok_or(PhaseBenchmarkError::InsufficientRuns)?;
    let phase_value = phase_result.ok_or(PhaseBenchmarkError::InsufficientRuns)?;
    if binary_value != phase_value {
        return Err(PhaseBenchmarkError::ResultMismatch);
    }

    let digit_count = bigint::digit_count(&binary_value);
    let result_hash = bigint::hash_value(&binary_value);
    let binary_stats = summarize(&binary_samples)?;
    let phase_stats = summarize(&phase_samples)?;
    let (label, speedup, reduction) =
        speedup_verdict(binary_stats.median_ns, phase_stats.median_ns);
    let alloc_reduction =
        allocation_reduction(binary_counters.allocations, phase_counters.allocations);

    let binary_linear = if include_linear {
        Some(build_engine_result(
            EngineKind::BinaryLinear,
            target,
            digit_count,
            result_hash,
            summarize(&[time_binary_linear(target).compute_ns])?,
            engines::fib_binary_linear(target).counters,
        ))
    } else {
        None
    };

    let full_pipeline = if include_full_pipeline {
        let pipeline = pipeline::run_full_pipeline(kernel, target);
        Some(pipeline.timing)
    } else {
        None
    };

    normalize_counter_averages(&mut binary_counters, repetitions);
    normalize_counter_averages(&mut phase_counters, repetitions);

    Ok(CompareReport {
        target,
        digit_count,
        repetitions,
        result_hash,
        result_equality: true,
        binary_linear,
        binary_fast: build_engine_result(
            EngineKind::BinaryFastDoubling,
            target,
            digit_count,
            result_hash,
            binary_stats,
            binary_counters,
        ),
        kairos_phase: build_engine_result(
            EngineKind::KairosPhaseDoubling,
            target,
            digit_count,
            result_hash,
            phase_stats,
            phase_counters,
        ),
        verdict: CompareVerdict {
            label,
            speedup,
            time_reduction_percent: reduction,
            allocation_reduction_percent: alloc_reduction,
        },
        full_pipeline,
    })
}

fn store_report(kernel: &mut Kernel, report: &CompareReport) {
    kernel.phase_benchmark.last_report = Some(report.clone());
    kernel.phase_benchmark.last_target = report.target;
    kernel.phase_benchmark.last_result_hash = report.result_hash;
    kernel.phase_benchmark.runs_completed += 1;
}

fn warmup(kernel: &mut Kernel, target: u64) {
    let tables = kernel.phase_math.tables.clone();
    let _ = engines::fib_binary_fast_doubling(target);
    let _ = engines::fib_kairos_phase_doubling(target, &tables);
}

fn time_binary_fast(target: u64) -> TimedOutput {
    let start = instant_now();
    let output = engines::fib_binary_fast_doubling(target);
    TimedOutput {
        output,
        compute_ns: instant_elapsed(start),
    }
}

fn time_binary_linear(target: u64) -> TimedOutput {
    let start = instant_now();
    let output = engines::fib_binary_linear(target);
    TimedOutput {
        output,
        compute_ns: instant_elapsed(start),
    }
}

fn time_kairos_phase(
    target: u64,
    tables: &crate::kernel::phase_math::tables::PhaseMathTables,
) -> TimedOutput {
    let start = instant_now();
    let output = engines::fib_kairos_phase_doubling(target, tables);
    TimedOutput {
        output,
        compute_ns: instant_elapsed(start),
    }
}

fn build_engine_result(
    engine: EngineKind,
    target: u64,
    digit_count: u64,
    result_hash: StateHash,
    compute_stats: crate::kernel::phase_benchmark::types::TimingStats,
    counters: EngineCounters,
) -> EngineRunResult {
    EngineRunResult {
        engine,
        target,
        digit_count,
        result_hash,
        compute_stats,
        full_stats: None,
        counters,
    }
}

fn merge_counters(total: &mut EngineCounters, sample: &EngineCounters) {
    total.recurrence_levels = sample.recurrence_levels;
    total.additions += sample.additions;
    total.subtractions += sample.subtractions;
    total.multiplications += sample.multiplications;
    total.branches += sample.branches;
    total.phase_lookups += sample.phase_lookups;
    total.compose_lookups += sample.compose_lookups;
    total.reflection_lookups += sample.reflection_lookups;
    total.topology_lookups += sample.topology_lookups;
    total.chi_lookups += sample.chi_lookups;
    total.transition_lookups += sample.transition_lookups;
    total.dynamic_branches += sample.dynamic_branches;
    total.state_transitions += sample.state_transitions;
    total.allocations += sample.allocations;
    total.allocated_bytes += sample.allocated_bytes;
    total.kairos_events += sample.kairos_events;
    total.trajectory_records += sample.trajectory_records;
}

fn normalize_counter_averages(counters: &mut EngineCounters, repetitions: u32) {
    let reps = repetitions as u64;
    if reps <= 1 {
        return;
    }
    counters.additions /= reps;
    counters.subtractions /= reps;
    counters.multiplications /= reps;
    counters.branches /= reps;
    counters.phase_lookups /= reps;
    counters.compose_lookups /= reps;
    counters.reflection_lookups /= reps;
    counters.topology_lookups /= reps;
    counters.chi_lookups /= reps;
    counters.transition_lookups /= reps;
    counters.dynamic_branches /= reps;
    counters.state_transitions /= reps;
    counters.allocations /= reps;
    counters.allocated_bytes /= reps;
    counters.kairos_events /= reps;
    counters.trajectory_records /= reps;
}

fn scale_workers_engine(
    target: u64,
    workers: usize,
    engine: EngineKind,
) -> Result<ScaleReport, PhaseBenchmarkError> {
    let worker_count = workers.max(1);
    let jobs = worker_count as u32;
    let tables = crate::kernel::phase_math::tables::PhaseMathTables::build();

    let single_start = instant_now();
    match engine {
        EngineKind::BinaryFastDoubling => {
            let _ = engines::fib_binary_fast_doubling(target);
        }
        EngineKind::KairosPhaseDoubling => {
            let _ = engines::fib_kairos_phase_doubling(target, &tables);
        }
        EngineKind::BinaryLinear => {
            let _ = engines::fib_binary_linear(target);
        }
    }
    let single_ns = instant_elapsed(single_start);

    let multi_start = instant_now();
    let mut handles = Vec::new();
    for w in 0..worker_count {
        let tables = tables.clone();
        let job_target = target + w as u64;
        handles.push(std::thread::spawn(move || match engine {
            EngineKind::BinaryFastDoubling => engines::fib_binary_fast_doubling(job_target),
            EngineKind::KairosPhaseDoubling => {
                engines::fib_kairos_phase_doubling(job_target, &tables)
            }
            EngineKind::BinaryLinear => engines::fib_binary_linear(job_target),
        }));
    }
    let mut sync_ops = 0u64;
    for handle in handles {
        sync_ops += 1;
        let _ = handle
            .join()
            .map_err(|_| PhaseBenchmarkError::WorkerError("worker join failed".into()))?;
    }
    let multi_ns = instant_elapsed(multi_start);

    let jobs_per_second = if multi_ns > 0 {
        jobs as f64 / (multi_ns as f64 / 1_000_000_000.0)
    } else {
        0.0
    };
    let efficiency = if worker_count > 0 && multi_ns > 0 {
        single_ns as f64 / (worker_count as f64 * multi_ns as f64)
    } else {
        0.0
    };

    Ok(ScaleReport {
        engine,
        base_target: target,
        worker_count,
        jobs,
        single_job_ns: single_ns,
        total_ns: multi_ns,
        jobs_per_second,
        sync_operations: sync_ops,
        efficiency,
    })
}

#[cfg(feature = "std")]
type BenchInstant = std::time::Instant;

#[cfg(feature = "std")]
fn instant_now() -> BenchInstant {
    std::time::Instant::now()
}

#[cfg(feature = "std")]
fn instant_elapsed(start: BenchInstant) -> u128 {
    start.elapsed().as_nanos()
}
