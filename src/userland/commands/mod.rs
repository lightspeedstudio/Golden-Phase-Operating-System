//! Terminal command handlers.

pub mod about;
pub mod anom_cmd;
pub mod clear;
pub mod clock_cmd;
pub mod dispatch;
pub mod dump;
pub mod g1p_cmd;
pub mod g64_cmd;
pub mod help;
pub mod host_cmd;
pub mod isa;
pub mod login_cmd;
pub mod mod_cmd;
pub mod passwd_cmd;
#[cfg(feature = "phaseguard-research")]
pub mod phaseguard_cmd;
pub mod regs;
pub mod run_paste;
pub mod selftest_cmd;
pub mod spawn_cmd;
pub mod state_cmd;
pub mod trace_cmd;
pub mod version;
pub mod whoami_cmd;
