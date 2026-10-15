//! P9 live media removal. This carrier has no SD0 card-detect (`HAS_CD=0`).
//! Yank is `MediaUnavailable`, never `NoCard` from `CARD_INSRT`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum P9Verdict {
    Pass,
    FailIo,
    FailNoUnavailable,
    FailPrompt,
    FailP9dbg,
    FailDurableAfterYank,
}

/// Historical PASS UART `190631` contains `P9DBG` from a temporary dump image.
/// `require_clean` is true only for a debug-free confirmation capture.
pub fn classify_p9_evidence(text: &str, require_clean: bool) -> P9Verdict {
    if require_clean && text.contains("P9DBG") {
        return P9Verdict::FailP9dbg;
    }
    if text.contains("journal i/o") {
        return P9Verdict::FailIo;
    }
    if !text.contains("gpos>") {
        return P9Verdict::FailPrompt;
    }
    if !text.contains("media unavailable") {
        return P9Verdict::FailNoUnavailable;
    }
    let after = text.rsplit_once("media unavailable").map(|(_, r)| r).unwrap_or("");
    if after.contains("durable: YES") && after.contains("SD_PARTITION_1") {
        return P9Verdict::FailDurableAfterYank;
    }
    P9Verdict::Pass
}
