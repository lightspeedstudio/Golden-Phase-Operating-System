//! P11: COMMIT payload, then wait for physical power cut.

use crate::platform::{String, Vec};

pub const P11_TOKEN: &str = "PHASEFS-P11-AFTER-COMMIT";
pub const P11_READY: &str = "P11_READY_FOR_PHYSICAL_POWER_CUT_AFTER_COMMIT";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum P11Fail {
    WrongToken,
    Io,
}

impl P11Fail {
    pub fn as_str(self) -> &'static str {
        match self {
            P11Fail::WrongToken => "wrong token",
            P11Fail::Io => "sd write/ready failure",
        }
    }
}

pub fn target_path(run_id: u64) -> String {
    format!("/tmp/p11_target_{run_id}.txt")
}

pub fn target_bytes(run_id: u64) -> Vec<u8> {
    format!("p11-must-survive-{run_id}").into_bytes()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum P11Verdict {
    Pass,
    FailTargetMissing,
    FailIo,
}

pub fn classify_p11_boot(text: &str, run_id: u64) -> P11Verdict {
    if text.contains("journal i/o") {
        return P11Verdict::FailIo;
    }
    let body = format!("p11-must-survive-{run_id}");
    if !text.contains(&body) {
        return P11Verdict::FailTargetMissing;
    }
    P11Verdict::Pass
}
