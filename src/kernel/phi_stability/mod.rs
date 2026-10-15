//! PHI-STABILITY — first full-stack KairOS stress test.

#![allow(clippy::too_many_arguments)]

pub mod dual_clock;
pub mod error;
pub mod fault;
pub mod fibonacci;
pub mod fixed;
pub mod merkle;
pub mod metrics;
pub mod network;
pub mod proof;
pub mod runner;
pub mod trajectory;
pub mod types;

pub use error::PhiStabilityError;
pub use runner::{
    fault_inject, fault_replay, network_receive, network_send, run_compare, run_local, status,
};
pub use types::PhiStabilityController;
