//! PHI-STABILITY metrics.

use crate::kernel::phi_stability::types::PhiStabilityResult;
use crate::platform::String;

#[derive(Debug, Default)]
pub struct PhiStabilityMetrics {
    pub jobs_submitted: u64,
    pub jobs_completed: u64,
    pub exact_matches: u64,
    pub mismatches: u64,
    pub max_trajectory_depth: u32,
    pub max_proof_queue: u32,
}

pub fn format_status(
    tests_completed: u64,
    tests_failed: u64,
    faults_detected: u64,
    faults_missed: u64,
    backend: &str,
    worker_count: usize,
    integer_mode: &str,
    last_target: u64,
    last_result_hash: &str,
    last_trajectory_root: &str,
    chronos_elapsed: u64,
    kairos_events: u64,
    invalid_transitions: u64,
    network_tests: u64,
    remote_matches: u64,
    replays_quarantined: u64,
    chain_ok: bool,
) -> String {
    format!(
        "PHI-STABILITY STATUS\n\n\
tests completed:          {tests_completed}\n\
tests failed:             {tests_failed}\n\
faults detected:          {faults_detected}\n\
faults missed:            {faults_missed}\n\n\
backend:                  {backend}\n\
worker count:             {worker_count}\n\
integer mode:             {integer_mode}\n\n\
last target:              F({last_target})\n\
last result hash:         {last_result_hash}\n\
last trajectory root:     {last_trajectory_root}\n\n\
Chronos elapsed:          {chronos_elapsed} ticks\n\
Kairos events:            {kairos_events}\n\
invalid transitions:      {invalid_transitions}\n\n\
network tests:            {network_tests}\n\
remote matches:           {remote_matches}\n\
replays quarantined:      {replays_quarantined}\n\
chain:                    {}\n",
        if chain_ok { "OK" } else { "FAIL" }
    )
}

pub fn format_run_result(result: &PhiStabilityResult) -> String {
    let preview = result.result_preview.as_deref().unwrap_or("<hash-only>");
    format!(
        "PHI-STABILITY\n\n\
test id:             {}\n\
target:              F({})\n\
algorithm:           fast doubling\n\
integer mode:        {}\n\
result:              {preview}\n\
result hash:         {}\n\
recurrence steps:    {}\n\n\
phase trajectory:    {}\n\
Chronos elapsed:     {}\n\
Kairos events:       {}\n\
statechain:          {}\n\
Merkle root:         {}\n",
        result.test_id.0,
        result.target_index,
        result.integer_mode.label(),
        crate::kernel::phi_stability::merkle::short_hash(&result.result_hash),
        result.recurrence_steps,
        if result.proof_valid { "OK" } else { "FAIL" },
        result.chronos_elapsed,
        result.kairos_events,
        if result.chain_valid { "OK" } else { "FAIL" },
        crate::kernel::phi_stability::merkle::short_hash(&result.merkle_root),
    )
}
