//! Phase-state benchmark report formatting.

use crate::kernel::phase_benchmark::stats::ns_to_ms;
use crate::kernel::phase_benchmark::types::{
    CompareReport, EngineKind, EngineRunResult, ScaleCompareReport, ScaleReport,
};
use crate::kernel::phi_stability::merkle::short_hash;
use crate::platform::String;

pub fn format_compare(report: &CompareReport) -> String {
    let mut out = String::new();
    out.push_str("KAIROS PHASE-STATE COMPUTATION BENCHMARK\n\n");
    out.push_str(&format!("target:                 F({})\n", report.target));
    out.push_str(&format!("digits:                 {}\n", report.digit_count));
    out.push_str(&format!("repetitions:            {}\n", report.repetitions));
    out.push_str("integer backend:        BigUint\n");
    out.push_str(&format!(
        "result equality:        {}\n\n",
        if report.result_equality {
            "MATCH"
        } else {
            "MISMATCH"
        }
    ));

    if let Some(linear) = &report.binary_linear {
        append_engine(&mut out, "BINARY LINEAR (reference)", linear);
        out.push('\n');
    }

    append_engine(&mut out, "BINARY FAST DOUBLING", &report.binary_fast);
    out.push('\n');
    append_engine(&mut out, "KAIROS PHASE DOUBLING", &report.kairos_phase);
    out.push_str("\nCOMPUTE RESULT\n");
    out.push_str(&format!(
        "verdict:                {}\n",
        report.verdict.label
    ));
    out.push_str(&format!(
        "speedup:                {:.3}x\n",
        report.verdict.speedup
    ));
    out.push_str(&format!(
        "time reduction:         {:.2}%\n",
        report.verdict.time_reduction_percent
    ));
    out.push_str(&format!(
        "allocation reduction:   {:.2}%\n",
        report.verdict.allocation_reduction_percent
    ));
    out.push_str(&format!(
        "exact result:           {}\n",
        if report.result_equality {
            "MATCH"
        } else {
            "MISMATCH"
        }
    ));

    if let Some(full) = &report.full_pipeline {
        out.push_str("\nFULL KAIROS PIPELINE\n");
        out.push_str(&format!(
            "compute:                {:.2} ms\n",
            ns_to_ms(full.compute_ns)
        ));
        out.push_str(&format!(
            "trajectory:             {:.2} ms\n",
            ns_to_ms(full.trajectory_ns)
        ));
        out.push_str(&format!(
            "proof:                  {:.2} ms\n",
            ns_to_ms(full.proof_ns)
        ));
        out.push_str(&format!(
            "Merkle:                 {:.2} ms\n",
            ns_to_ms(full.merkle_ns)
        ));
        out.push_str(&format!(
            "statechain:             {:.2} ms\n",
            ns_to_ms(full.statechain_ns)
        ));
        out.push_str(&format!(
            "total:                  {:.2} ms\n",
            ns_to_ms(full.total_ns)
        ));
    }

    out.push_str(&format!(
        "\nresult hash:            {}\n",
        short_hash(&report.result_hash)
    ));
    out
}

fn append_engine(out: &mut String, title: &str, run: &EngineRunResult) {
    out.push_str(title);
    out.push('\n');
    out.push_str(&format!(
        "median compute time:    {:.2} ms\n",
        ns_to_ms(run.compute_stats.median_ns)
    ));
    out.push_str(&format!(
        "minimum:                {:.2} ms\n",
        ns_to_ms(run.compute_stats.min_ns)
    ));
    out.push_str(&format!(
        "maximum:                {:.2} ms\n",
        ns_to_ms(run.compute_stats.max_ns)
    ));
    out.push_str(&format!(
        "stddev:                 {:.2} ms\n",
        ns_to_ms(run.compute_stats.stddev_ns as u128)
    ));
    out.push_str(&format!(
        "coefficient of var:     {:.2}%\n",
        run.compute_stats.cv_percent
    ));
    out.push_str(&format!(
        "branches:               {}\n",
        run.counters.branches
    ));
    if run.engine == EngineKind::KairosPhaseDoubling {
        out.push_str(&format!(
            "phase lookups:          {}\n",
            run.counters.phase_lookups
        ));
        out.push_str(&format!(
            "dynamic branches:       {}\n",
            run.counters.dynamic_branches
        ));
        out.push_str(&format!(
            "compose lookups:        {}\n",
            run.counters.compose_lookups
        ));
        out.push_str(&format!(
            "reflection lookups:     {}\n",
            run.counters.reflection_lookups
        ));
        out.push_str(&format!(
            "topology lookups:       {}\n",
            run.counters.topology_lookups
        ));
        out.push_str(&format!(
            "chi lookups:            {}\n",
            run.counters.chi_lookups
        ));
        out.push_str(&format!(
            "legal-transition lookups: {}\n",
            run.counters.transition_lookups
        ));
        out.push_str(&format!(
            "state transitions:      {}\n",
            run.counters.state_transitions
        ));
        out.push_str(&format!(
            "trajectory records:     {}\n",
            run.counters.trajectory_records
        ));
        out.push_str(&format!(
            "Kairos events:          {}\n",
            run.counters.kairos_events
        ));
    }
    out.push_str(&format!(
        "big-int multiplies:     {}\n",
        run.counters.multiplications
    ));
    out.push_str(&format!(
        "allocations:            {}\n",
        run.counters.allocations
    ));
    out.push_str(&format!(
        "recurrence levels:      {}\n",
        run.counters.recurrence_levels
    ));
}

pub fn format_status(
    runs_completed: u64,
    last_target: u64,
    last_hash: &crate::kernel::hash::StateHash,
    repetitions: u32,
    workers: usize,
) -> String {
    format!(
        "PHASE-STATE BENCHMARK STATUS\n\n\
engine:                 KAIROS-PHASE-DOUBLING vs BINARY-FAST-DOUBLING\n\
integer backend:        BigUint (hosted)\n\
default repetitions:    {repetitions}\n\
phase math workers:     {workers}\n\
runs completed:         {runs_completed}\n\
last target:            F({last_target})\n\
last result hash:       {}\n\
primary command:        phase-benchmark compare 1000000\n",
        short_hash(last_hash)
    )
}

pub fn format_scale(report: &ScaleReport) -> String {
    format!(
        "PHASE-STATE BENCHMARK SCALE\n\n\
engine:                 {}\n\
base target:            F({})\n\
workers:                {}\n\
jobs:                   {}\n\
single job:             {:.2} ms\n\
batch total:            {:.2} ms\n\
jobs per second:        {:.2}\n\
sync operations:        {}\n\
scaling efficiency:     {:.3}\n",
        report.engine.label(),
        report.base_target,
        report.worker_count,
        report.jobs,
        ns_to_ms(report.single_job_ns),
        ns_to_ms(report.total_ns),
        report.jobs_per_second,
        report.sync_operations,
        report.efficiency
    )
}

pub fn format_scale_compare(report: &ScaleCompareReport) -> String {
    let b = &report.binary_fast;
    let p = &report.kairos_phase;
    let speedup = if p.total_ns > 0 {
        b.total_ns as f64 / p.total_ns as f64
    } else {
        1.0
    };
    format!(
        "PHASE-STATE BENCHMARK SCALE COMPARE\n\n\
target:                 F({})\n\
workers:                {}\n\
result equality:        {}\n\n\
BINARY-FAST-DOUBLING\n\
single job:             {:.2} ms\n\
batch total:            {:.2} ms\n\
jobs per second:        {:.2}\n\
scaling efficiency:     {:.3}\n\n\
KAIROS-PHASE-DOUBLING\n\
single job:             {:.2} ms\n\
batch total:            {:.2} ms\n\
jobs per second:        {:.2}\n\
scaling efficiency:     {:.3}\n\n\
PARALLEL RESULT\n\
batch speedup:          {:.3}x (binary ÷ phase)\n",
        report.target,
        report.worker_count,
        if report.result_equality {
            "MATCH"
        } else {
            "MISMATCH"
        },
        ns_to_ms(b.single_job_ns),
        ns_to_ms(b.total_ns),
        b.jobs_per_second,
        b.efficiency,
        ns_to_ms(p.single_job_ns),
        ns_to_ms(p.total_ns),
        p.jobs_per_second,
        p.efficiency,
        speedup
    )
}

pub fn format_sweep(reports: &[CompareReport]) -> String {
    let mut out = String::from("PHASE-STATE BENCHMARK SWEEP\n\n");
    for report in reports {
        out.push_str(&format!(
            "F({}) digits={} speedup={:.3}x verdict={}\n",
            report.target, report.digit_count, report.verdict.speedup, report.verdict.label
        ));
    }
    out
}

pub fn format_dual_clock(
    report: &CompareReport,
    chronos_compute_ns: u128,
    kairos_events: u64,
) -> String {
    format!(
        "{}\nDUAL-CLOCK SUMMARY\n\
Chronos compute median: {:.2} ms\n\
Kairos events (phase):  {}\n",
        format_compare(report),
        ns_to_ms(chronos_compute_ns),
        kairos_events
    )
}

pub fn format_network_compare(
    peer: &str,
    local: &CompareReport,
    remote_speedup: Option<f64>,
) -> String {
    let mut out = format!(
        "PHASE-STATE NETWORK COMPARE\n\n\
peer:                   {peer}\n\
local target:           F({})\n\
local result hash:      {}\n\
local speedup:          {:.3}x\n",
        local.target,
        short_hash(&local.result_hash),
        local.verdict.speedup
    );
    if let Some(remote) = remote_speedup {
        out.push_str(&format!("remote speedup:         {remote:.3}x\n"));
    } else {
        out.push_str("remote speedup:         (awaiting peer measurement)\n");
    }
    out.push_str(&format!(
        "trajectory root:        {}\n",
        short_hash(&local.result_hash)
    ));
    out
}
