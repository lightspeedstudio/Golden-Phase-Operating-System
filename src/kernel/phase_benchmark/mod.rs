//! KairOS phase-state computational benchmark (hosted BigUint).

pub mod bigint;
pub mod engines;
pub mod error;
pub mod pipeline;
pub mod report;
pub mod runner;
pub mod stats;
pub mod types;

pub use error::PhaseBenchmarkError;
pub use runner::{
    compare_inner, last_report, run_compare, run_dual_clock, run_network_compare, run_repeat,
    run_scale, run_single, run_sweep, scale_benchmark, scale_benchmark_compare, status,
};
pub use types::PhaseBenchmarkController;
