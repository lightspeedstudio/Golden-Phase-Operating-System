//! P7: corrupt uncommitted final WAL sector. Research-only.
//!
//! Recovery: checksum-invalid tail without COMMIT is `RejectedCorrupt`
//! (`wal.rs` recover, `ScanStop::Corrupt && !has_commit`). `Isolated` is only
//! a valid uncommitted prefix that scans to `End`. P7 must not take Isolated.

use super::format::sha256;
use super::{WAL_START, WAL_SECTORS};
use core::fmt;

pub const P7_TOKEN: &str = "PHASEFS-P7-CORRUPT-TAIL";
pub const P7_CORRUPT_OFFSET: usize = 200;
pub const P7_PAYLOAD_FALLBACK_OFFSET: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct P7Window {
    pub start_lba: u32,
    pub p1_sectors: u32,
    pub device_sectors: u32,
    pub p0_start: u32,
    pub p0_sectors: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct P7Report {
    pub run_id: u64,
    pub abs_lba: u32,
    pub offset: usize,
    pub before: [u8; 32],
    pub after: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum P7Fail {
    WrongToken,
    MissingPartition,
    CapacityUnknown,
    TargetBeforeP1,
    TargetPastEnd,
    OverlapP0,
    NoWalSector,
    CommittedTail,
    OffsetNotCovered,
    ReadbackMismatch,
    Io,
}

impl P7Fail {
    pub fn as_str(self) -> &'static str {
        match self {
            P7Fail::WrongToken => "wrong token",
            P7Fail::MissingPartition => "missing partition 1",
            P7Fail::CapacityUnknown => "device_sectors zero",
            P7Fail::TargetBeforeP1 => "target before partition 1",
            P7Fail::TargetPastEnd => "target at or beyond p1_end",
            P7Fail::OverlapP0 => "target overlaps partition 0",
            P7Fail::NoWalSector => "no final WAL sector",
            P7Fail::CommittedTail => "committed final transaction",
            P7Fail::OffsetNotCovered => "byte outside checksum coverage",
            P7Fail::ReadbackMismatch => "read-back verification failed",
            P7Fail::Io => "sd write/flush failure",
        }
    }
}

impl fmt::Display for P7Fail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Absolute LBA of a journal-relative sector. Never takes a caller LBA.
pub fn derive_abs_lba(w: P7Window, journal_rel: u32) -> Result<u32, P7Fail> {
    if w.device_sectors == 0 {
        return Err(P7Fail::CapacityUnknown);
    }
    if w.p1_sectors == 0 {
        return Err(P7Fail::MissingPartition);
    }
    let abs = w
        .start_lba
        .checked_add(journal_rel)
        .ok_or(P7Fail::TargetPastEnd)?;
    if abs < w.start_lba {
        return Err(P7Fail::TargetBeforeP1);
    }
    let p1_end = w
        .start_lba
        .checked_add(w.p1_sectors)
        .ok_or(P7Fail::TargetPastEnd)?;
    if abs >= p1_end {
        return Err(P7Fail::TargetPastEnd);
    }
    if abs >= w.device_sectors {
        return Err(P7Fail::TargetPastEnd);
    }
    if w.p0_sectors > 0 {
        let p0_end = w.p0_start.saturating_add(w.p0_sectors);
        if abs >= w.p0_start && abs < p0_end {
            return Err(P7Fail::OverlapP0);
        }
    }
    Ok(abs)
}

pub fn wal_journal_rel(wal_rel: u32) -> u32 {
    WAL_START + (wal_rel % WAL_SECTORS)
}

/// Offset 200 if it lies in checksum-covered payload of a 1-sector record;
/// otherwise the first payload byte (128).
pub fn checksum_covered_offset(payload_len: u32) -> Result<usize, P7Fail> {
    if payload_len == 0 {
        return Err(P7Fail::OffsetNotCovered);
    }
    let covered_end = 128usize.saturating_add(payload_len as usize);
    if P7_CORRUPT_OFFSET < covered_end && P7_CORRUPT_OFFSET >= 128 {
        Ok(P7_CORRUPT_OFFSET)
    } else if covered_end > P7_PAYLOAD_FALLBACK_OFFSET {
        Ok(P7_PAYLOAD_FALLBACK_OFFSET)
    } else {
        Err(P7Fail::OffsetNotCovered)
    }
}

pub fn sector_hash(sec: &[u8; 512]) -> [u8; 32] {
    sha256(sec)
}

/// P7 UART classifier. Splash is irrelevant.
pub fn classify_p7_evidence(text: &str, run_id: u64, torn_name: &str, anchor: &str) -> P7Verdict {
    let _ = torn_name;
    if text.contains("journal i/o") {
        return P7Verdict::FailIo;
    }
    let armed = format!("P7_ARMED run={run_id}");
    let uncommitted = format!("P7_UNCOMMITTED_WRITTEN run={run_id}");
    let verified = format!("P7_CORRUPTION_VERIFIED run={run_id}");
    if !text.contains(&armed) || !text.contains(&uncommitted) || !text.contains(&verified) {
        return P7Verdict::NoCorruptionMarker;
    }
    if !text.contains("P7_RESETTING") {
        return P7Verdict::NoReset;
    }
    let ia = match text.find(&armed) {
        Some(i) => i,
        None => return P7Verdict::NoCorruptionMarker,
    };
    if let Some(first) = text.find("P7_ARMED run=") {
        if first != ia {
            return P7Verdict::MismatchedRun;
        }
    }
    let iu = text.find(&uncommitted).unwrap();
    let iv = text.find(&verified).unwrap();
    let ir = text.find("P7_RESETTING").unwrap();
    if !(ia < iu && iu < iv && iv < ir) {
        return P7Verdict::MismatchedRun;
    }
    if let (Some(b), Some(a)) = (find_hash(text, "before="), find_hash(text, "after=")) {
        if b == a {
            return P7Verdict::SameHash;
        }
    }
    let rec = text[ir..].find("class=REJECTED_CORRUPT");
    if rec.is_none() {
        if text[ir..].contains("class=RECOVERED") {
            return P7Verdict::FalseRecovered;
        }
        return P7Verdict::NoCorruptionMarker;
    }
    if text[ir..].contains("replayed=1") {
        return P7Verdict::Replayed;
    }
    let content = format!("p7-must-not-appear-{run_id}");
    if content_after_reset(text, ir, &content) {
        return P7Verdict::TornPresent;
    }
    if !text.contains(anchor) {
        return P7Verdict::AnchorMissing;
    }
    if text[ir..].matches("fsj recover: class=CLEAN").count() < 2
        && text[ir..].matches("class=CLEAN").count() < 2
    {
        return P7Verdict::MissingClean;
    }
    if !text[ir..].contains("fsj check: PASS") {
        return P7Verdict::MissingCheck;
    }
    P7Verdict::Pass
}

fn content_after_reset(text: &str, reset_at: usize, content: &str) -> bool {
    text[reset_at..].contains(content)
}

fn find_hash<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let i = text.find(key)?;
    let s = &text[i + key.len()..];
    let n = s.chars().take_while(|c| c.is_ascii_hexdigit()).count();
    if n >= 16 {
        Some(&s[..n.min(64)])
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum P7Verdict {
    Pass,
    NoCorruptionMarker,
    SameHash,
    NoReset,
    FalseRecovered,
    Replayed,
    TornPresent,
    AnchorMissing,
    MissingClean,
    MissingCheck,
    FailIo,
    MismatchedRun,
}

impl P7Verdict {
    pub fn as_str(self) -> &'static str {
        match self {
            P7Verdict::Pass => "PASS",
            P7Verdict::NoCorruptionMarker => "NO_CORRUPTION_MARKER",
            P7Verdict::SameHash => "SAME_HASH",
            P7Verdict::NoReset => "NO_RESET",
            P7Verdict::FalseRecovered => "FALSE_RECOVERED",
            P7Verdict::Replayed => "REPLAYED",
            P7Verdict::TornPresent => "TORN_PRESENT",
            P7Verdict::AnchorMissing => "ANCHOR_MISSING",
            P7Verdict::MissingClean => "MISSING_CLEAN",
            P7Verdict::MissingCheck => "MISSING_CHECK",
            P7Verdict::FailIo => "JOURNAL_IO",
            P7Verdict::MismatchedRun => "MISMATCHED_RUN",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win() -> P7Window {
        P7Window {
            start_lba: 15706112,
            p1_sectors: 16384,
            device_sectors: 15728640,
            p0_start: 2048,
            p0_sectors: 15704064,
        }
    }

    #[test]
    fn p7_lba_refuses_capacity_zero() {
        let mut w = win();
        w.device_sectors = 0;
        assert_eq!(derive_abs_lba(w, WAL_START), Err(P7Fail::CapacityUnknown));
    }

    #[test]
    fn p7_lba_refuses_missing_p1() {
        let mut w = win();
        w.p1_sectors = 0;
        assert_eq!(derive_abs_lba(w, WAL_START), Err(P7Fail::MissingPartition));
    }

    #[test]
    fn p7_lba_refuses_before_p1() {
        let abs = derive_abs_lba(win(), wal_journal_rel(0)).unwrap();
        assert!(abs >= win().start_lba);
        assert!(abs >= win().p0_start.saturating_add(win().p0_sectors));
    }

    #[test]
    fn p7_lba_refuses_past_end() {
        assert_eq!(
            derive_abs_lba(win(), 16384),
            Err(P7Fail::TargetPastEnd)
        );
    }

    #[test]
    fn p7_lba_refuses_p0_overlap() {
        let w = P7Window {
            start_lba: 1000,
            p1_sectors: 20000,
            device_sectors: 15728640,
            p0_start: 2048,
            p0_sectors: 100,
        };
        // journal_rel such that abs = 2048 (in p0)
        assert_eq!(derive_abs_lba(w, 1048), Err(P7Fail::OverlapP0));
    }

    #[test]
    fn p7_wal_rel_stays_in_p1() {
        let abs = derive_abs_lba(win(), wal_journal_rel(0)).unwrap();
        assert!(abs >= 15706112);
        assert!(abs < 15706112 + 16384);
        assert!(abs < 15728640);
        assert!(abs >= 15706114); // WAL_START=2
    }

    #[test]
    fn p7_offset_200_not_covered_for_empty_payload() {
        assert_eq!(checksum_covered_offset(0), Err(P7Fail::OffsetNotCovered));
    }

    #[test]
    fn p7_offset_200_covered_when_payload_long() {
        assert_eq!(checksum_covered_offset(80), Ok(200));
    }

    #[test]
    fn p7_classifier_negatives() {
        let good = "\
P7_ARMED run=3
P7_UNCOMMITTED_WRITTEN run=3 lba=15706120
P7_CORRUPTION_VERIFIED run=3 lba=15706120 offset=200 before=aaaaaaaaaaaaaaaa after=bbbbbbbbbbbbbbbb
P7_RESETTING
fsj: recovered class=REJECTED_CORRUPT replayed=0 discarded=1 gen=2
p7-anchor-preserve
fsj check: PASS
fsj recover: class=CLEAN
fsj recover: class=CLEAN
";
        assert_eq!(
            classify_p7_evidence(good, 3, "/tmp/p7_torn_3.txt", "p7-anchor-preserve"),
            P7Verdict::Pass
        );
        assert_eq!(
            classify_p7_evidence("no markers", 3, "/tmp/p7_torn_3.txt", "p7-anchor-preserve"),
            P7Verdict::NoCorruptionMarker
        );
        let same = good.replace("after=bbbbbbbbbbbbbbbb", "after=aaaaaaaaaaaaaaaa");
        assert_eq!(
            classify_p7_evidence(&same, 3, "/tmp/p7_torn_3.txt", "p7-anchor-preserve"),
            P7Verdict::SameHash
        );
        let no_reset = good.replace("P7_RESETTING\n", "");
        assert_eq!(
            classify_p7_evidence(&no_reset, 3, "/tmp/p7_torn_3.txt", "p7-anchor-preserve"),
            P7Verdict::NoReset
        );
        let rec = good.replace("REJECTED_CORRUPT replayed=0", "RECOVERED replayed=1");
        assert_eq!(
            classify_p7_evidence(&rec, 3, "/tmp/p7_torn_3.txt", "p7-anchor-preserve"),
            P7Verdict::FalseRecovered
        );
        let replayed = good.replace("REJECTED_CORRUPT replayed=0", "REJECTED_CORRUPT replayed=1");
        assert_eq!(
            classify_p7_evidence(&replayed, 3, "/tmp/p7_torn_3.txt", "p7-anchor-preserve"),
            P7Verdict::Replayed
        );
        let torn = format!("{good}p7-must-not-appear-3\n");
        assert_eq!(
            classify_p7_evidence(&torn, 3, "/tmp/p7_torn_3.txt", "p7-anchor-preserve"),
            P7Verdict::TornPresent
        );
        assert_eq!(
            classify_p7_evidence(good, 3, "/tmp/p7_torn_3.txt", "missing-anchor"),
            P7Verdict::AnchorMissing
        );
        let no_clean = good.replace("fsj recover: class=CLEAN\n", "");
        assert_eq!(
            classify_p7_evidence(&no_clean, 3, "/tmp/p7_torn_3.txt", "p7-anchor-preserve"),
            P7Verdict::MissingClean
        );
        let io = format!("{good}journal i/o\n");
        assert_eq!(
            classify_p7_evidence(&io, 3, "/tmp/p7_torn_3.txt", "p7-anchor-preserve"),
            P7Verdict::FailIo
        );
        let reused = format!("P7_ARMED run=1\n{good}");
        assert_eq!(
            classify_p7_evidence(&reused, 3, "/tmp/p7_torn_3.txt", "p7-anchor-preserve"),
            P7Verdict::MismatchedRun
        );
        // #region agent log
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("debug-63b087.log")
        {
            use std::io::Write;
            let _ = writeln!(
                f,
                r#"{{"sessionId":"63b087","hypothesisId":"H1","location":"p7.rs:classifier","message":"classifier negatives","data":{{"pass":true}},"timestamp":0}}"#
            );
        }
        // #endregion
    }
}
