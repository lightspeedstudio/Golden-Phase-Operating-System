//! P10: persist BEGIN+payload, no COMMIT, then wait for physical power cut.

use crate::platform::{String, Vec};

pub const P10_TOKEN: &str = "PHASEFS-P10-BEFORE-COMMIT";
pub const P10_ANCHOR_PATH: &str = "/tmp/p10_anchor.txt";
pub const P10_ANCHOR_BYTES: &[u8] = b"p10-anchor-preserve";
pub const P10_READY: &str = "P10_READY_FOR_PHYSICAL_POWER_CUT_BEFORE_COMMIT";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum P10Fail {
    WrongToken,
    Io,
}

impl P10Fail {
    pub fn as_str(self) -> &'static str {
        match self {
            P10Fail::WrongToken => "wrong token",
            P10Fail::Io => "sd write/ready failure",
        }
    }
}

pub fn target_path(run_id: u64) -> String {
    format!("/tmp/p10_target_{run_id}.txt")
}

pub fn target_bytes(run_id: u64) -> Vec<u8> {
    format!("p10-must-not-commit-{run_id}").into_bytes()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum P10Verdict {
    Pass,
    FailTargetPresent,
    FailAnchor,
    FailIo,
    FailFalseRecovered,
}

pub fn classify_p10_boot(text: &str, run_id: u64) -> P10Verdict {
    if text.contains("journal i/o") {
        return P10Verdict::FailIo;
    }
    let tpath = format!("/tmp/p10_target_{run_id}.txt");
    if text.contains("RECOVERED") && text.contains(&tpath) {
        return P10Verdict::FailFalseRecovered;
    }
    if text.contains(&tpath) && !text.contains("ISOLATED") {
        return P10Verdict::FailTargetPresent;
    }
    P10Verdict::Pass
}
