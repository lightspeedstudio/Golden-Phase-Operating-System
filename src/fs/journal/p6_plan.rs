//! One-shot P6 recovery crash plan. Lives at relative LBA `P6_PLAN_REL`,
//! after WAL/checkpoint (`REGION_SECTORS`), never in superblock or WAL.

use super::format::sha256;
use super::{JournalError, REGION_SECTORS};

/// First sector past the 6146-sector journal region.
pub const P6_PLAN_REL: u32 = REGION_SECTORS;
pub const P6_PLAN_SECTORS: u32 = 1;
pub const P6_PLAN_MAGIC: [u8; 4] = *b"GP6P";
pub const P6_PLAN_VERSION: u16 = 1;
/// Fire immediately before REDO apply on COMMIT-without-APPLIED recovery.
pub const P6_STAGE_BEFORE_REPLAY_APPLY: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct P6Plan {
    pub run_id: u64,
    pub stage: u32,
    pub armed: u8,
    pub remaining: u8,
    pub generation: u64,
    pub txn_id: u64,
}

impl P6Plan {
    pub fn armed(run_id: u64, generation: u64, txn_id: u64) -> Self {
        Self {
            run_id,
            stage: P6_STAGE_BEFORE_REPLAY_APPLY,
            armed: 1,
            remaining: 1,
            generation,
            txn_id,
        }
    }

    pub fn consumed(self) -> Self {
        Self {
            armed: 0,
            remaining: 0,
            ..self
        }
    }
}

pub fn encode_plan(p: &P6Plan) -> [u8; 512] {
    let mut s = [0u8; 512];
    s[0..4].copy_from_slice(&P6_PLAN_MAGIC);
    s[4..6].copy_from_slice(&P6_PLAN_VERSION.to_le_bytes());
    s[8..16].copy_from_slice(&p.run_id.to_le_bytes());
    s[16..20].copy_from_slice(&p.stage.to_le_bytes());
    s[20] = p.armed;
    s[21] = p.remaining;
    s[24..32].copy_from_slice(&p.generation.to_le_bytes());
    s[32..40].copy_from_slice(&p.txn_id.to_le_bytes());
    let sum = sha256(&s[0..480]);
    s[480..512].copy_from_slice(&sum);
    s
}

pub fn decode_plan(sec: &[u8; 512]) -> Result<P6Plan, JournalError> {
    if sec[0..4] != P6_PLAN_MAGIC {
        return Err(JournalError::BadMagic);
    }
    let ver = u16::from_le_bytes(sec[4..6].try_into().unwrap());
    if ver != P6_PLAN_VERSION {
        return Err(JournalError::BadVersion);
    }
    let mut chk = *sec;
    for b in chk[480..512].iter_mut() {
        *b = 0;
    }
    let mut stored = [0u8; 32];
    stored.copy_from_slice(&sec[480..512]);
    if sha256(&chk[0..480]) != stored {
        return Err(JournalError::Checksum);
    }
    Ok(P6Plan {
        run_id: u64::from_le_bytes(sec[8..16].try_into().unwrap()),
        stage: u32::from_le_bytes(sec[16..20].try_into().unwrap()),
        armed: sec[20],
        remaining: sec[21],
        generation: u64::from_le_bytes(sec[24..32].try_into().unwrap()),
        txn_id: u64::from_le_bytes(sec[32..40].try_into().unwrap()),
    })
}

fn parse_u64_at(s: &str) -> Option<u64> {
    let mut n = 0u64;
    let mut any = false;
    for c in s.chars() {
        if !c.is_ascii_digit() {
            break;
        }
        any = true;
        n = n.saturating_mul(10).saturating_add((c as u8 - b'0') as u64);
    }
    if any {
        Some(n)
    } else {
        None
    }
}

fn run_after(text: &str, marker: &str) -> Option<u64> {
    let i = text.find(marker)?;
    let rest = &text[i..];
    let r = rest.find("run=")?;
    parse_u64_at(&rest[r + 4..])
}

fn distinct_crash_points(text: &str) -> usize {
    let mut seen = [false; 128];
    let mut n = 0usize;
    let key = "fsj CRASH point=";
    let mut rest = text;
    while let Some(i) = rest.find(key) {
        let after = &rest[i + key.len()..];
        if let Some(p) = parse_u64_at(after) {
            let idx = (p as usize) % seen.len();
            if !seen[idx] {
                seen[idx] = true;
                n += 1;
            }
        }
        rest = &rest[i + key.len()..];
    }
    n
}

/// TEST_MATRIX P6 classifier. Does not require a boot splash on Boot B.
pub fn classify_p6_transcript(text: &str) -> P6Verdict {
    classify_p6_evidence(text, None)
}

pub fn classify_p6_evidence(text: &str, expected_payload: Option<&str>) -> P6Verdict {
    if text.contains("journal i/o") {
        return P6Verdict::FailIo;
    }
    let armed_run = run_after(text, "P6_ARMED");
    let committed_run = run_after(text, "P6_TRANSACTION_COMMITTED");
    let begin_run = run_after(text, "P6_RECOVERY_BEGIN");
    let fired_run = run_after(text, "P6_RECOVERY_CRASH_FIRED");
    if fired_run.is_none() {
        return P6Verdict::OpenNoResetDuringRecovery;
    }
    if distinct_crash_points(text) < 2 {
        return P6Verdict::OpenNoResetDuringRecovery;
    }
    let Some(run) = armed_run else {
        return P6Verdict::MismatchedRun;
    };
    if committed_run != Some(run) || begin_run != Some(run) || fired_run != Some(run) {
        return P6Verdict::MismatchedRun;
    }
    let armed = text.find("P6_ARMED").unwrap();
    let committed = text.find("P6_TRANSACTION_COMMITTED").unwrap();
    let begin = text.find("P6_RECOVERY_BEGIN").unwrap();
    let fired = text.find("P6_RECOVERY_CRASH_FIRED").unwrap();
    let recovered = text[fired..]
        .find("recovered class=RECOVERED replayed=1")
        .map(|i| fired + i);
    let check = text[fired..]
        .find("fsj check: PASS")
        .map(|i| fired + i);
    let clean = recovered.and_then(|r| {
        text[r..]
            .find("fsj recover: class=CLEAN")
            .map(|i| r + i)
    });
    if recovered.is_none() {
        return P6Verdict::MissingRecoveredReplay;
    }
    if check.is_none() {
        return P6Verdict::MissingCheck;
    }
    if clean.is_none() {
        return P6Verdict::MissingClean;
    }
    if !(armed < committed && committed < begin && begin < fired) {
        return P6Verdict::ReusedOrUnordered;
    }
    if expected_payload.is_some_and(|p| !text.contains(p)) {
        return P6Verdict::MissingFile;
    }
    P6Verdict::Pass
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum P6Verdict {
    Pass,
    MarkersPresent,
    OpenNoResetDuringRecovery,
    FailIo,
    MismatchedRun,
    MissingFile,
    MissingClean,
    MissingRecoveredReplay,
    MissingCheck,
    ReusedOrUnordered,
}

impl P6Verdict {
    pub fn as_str(self) -> &'static str {
        match self {
            P6Verdict::Pass | P6Verdict::MarkersPresent => "PASS",
            P6Verdict::OpenNoResetDuringRecovery => "NO_RESET_DURING_RECOVERY",
            P6Verdict::FailIo => "JOURNAL_IO",
            P6Verdict::MismatchedRun => "MISMATCHED_RUN",
            P6Verdict::MissingFile => "MISSING_FILE",
            P6Verdict::MissingClean => "MISSING_CLEAN",
            P6Verdict::MissingRecoveredReplay => "MISSING_RECOVERED_REPLAY",
            P6Verdict::MissingCheck => "MISSING_CHECK",
            P6Verdict::ReusedOrUnordered => "REUSED_OR_UNORDERED",
        }
    }
}
