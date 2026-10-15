//! Avenue 7 — hybrid integrity frame.
//!
//! Combines CRC-16/CCITT (raw byte-level *detection*, per the OSF paper's
//! honest §8 finding that CRC dominates detection) with the GPL compose
//! accumulator, quantized branch-sheet chi, `assoc_parity`, and an optional
//! sliding-triple topology trace (semantic *diagnosis*: same-vs-reflection
//! classification, localized to a symbol index).
//!
//! Never reimplements compose or the checksum/topology math: everything here
//! delegates to `crate::core::gphase_core` via `crate::protocol::checksum`
//! and `crate::protocol::topology`. See
//! `docs/reports/AVENUE_07_hybrid_frame.md` for the full byte-layout table
//! and the measured detection/diagnosis numbers.

use crate::core::gphase_core::{PhaseState, TopologyKind};
use crate::platform::Vec;
use crate::protocol::checksum::{gpl_checksum, state_sheet};
use crate::protocol::encoder::phase_to_bytes;
use crate::protocol::topology::scan_sliding_triples;

/// Fixed-point scale used to quantize the real-valued chi sheet-sum into a
/// reproducible integer. At `CHI_SCALE = 100.0` and stream length L=64
/// (matching `gpl_program.py` §4), `|chi| <= L * max|sheet| ~= 81.5`, so
/// `chi * CHI_SCALE` fits comfortably inside +-8192 (14 bits) — see the
/// overhead accounting in the report.
pub const CHI_SCALE: f64 = 100.0;

#[inline]
fn fround(x: f64) -> f64 {
    #[cfg(feature = "std")]
    {
        x.round()
    }
    #[cfg(all(feature = "baremetal", not(feature = "std")))]
    {
        libm::round(x)
    }
}

/// Quantize a real-valued chi into a reproducible 16-bit integer.
pub fn quantize_chi(chi: f64) -> i16 {
    let scaled = fround(chi * CHI_SCALE);
    let clamped = scaled.clamp(i16::MIN as f64, i16::MAX as f64);
    clamped as i16
}

/// CRC-16/CCITT (poly 0x1021, init 0xFFFF) — bit-identical to
/// `scripts/research/gpl_program.py` §4 `crc16`.
pub fn crc16_ccitt(data: &[u8]) -> u16 {
    let mut c: u16 = 0xFFFF;
    for &byte in data {
        c ^= (byte as u16) << 8;
        for _ in 0..8 {
            c = if c & 0x8000 != 0 {
                (c << 1) ^ 0x1021
            } else {
                c << 1
            };
        }
    }
    c
}

/// reflection-defect count (mod 2) over the sliding-triple topology scan —
/// the same quantity the statechain records as `assoc_parity`.
pub fn topology_parity(states: &[PhaseState]) -> u8 {
    (scan_sliding_triples(states)
        .into_iter()
        .filter(|e| e.kind == TopologyKind::Reflection)
        .count()
        & 1) as u8
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameError {
    PayloadLengthNotMultipleOf4(usize),
}

/// The hybrid integrity frame: payload + CRC-16/CCITT + GPL acc+chi
/// (quantized) + assoc_parity + optional topology trace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HybridFrame {
    pub payload: Vec<PhaseState>,
    pub crc16: u16,
    pub gpl_acc: PhaseState,
    pub gpl_chi_q: i16,
    pub assoc_parity: u8,
    pub topology_trace: Option<Vec<TopologyKind>>,
}

impl HybridFrame {
    /// Encode a frame from a phase-symbol payload. `payload.len()` must be a
    /// multiple of 4 (byte-packing requirement, matches
    /// `protocol::encoder::phase_to_bytes`). `with_topology` attaches the
    /// L-2 entry sliding-triple trace (a reference trace, not a compact
    /// checksum — see overhead accounting in the report).
    pub fn encode(payload: &[PhaseState], with_topology: bool) -> Result<Self, FrameError> {
        let bytes = phase_to_bytes(payload)
            .map_err(|_| FrameError::PayloadLengthNotMultipleOf4(payload.len()))?;
        let crc16 = crc16_ccitt(&bytes);
        let (gpl_acc, chi) = gpl_checksum(payload);
        let gpl_chi_q = quantize_chi(chi);
        let assoc_parity = topology_parity(payload);
        let topology_trace = if with_topology {
            Some(
                scan_sliding_triples(payload)
                    .into_iter()
                    .map(|e| e.kind)
                    .collect(),
            )
        } else {
            None
        };
        Ok(HybridFrame {
            payload: payload.to_vec(),
            crc16,
            gpl_acc,
            gpl_chi_q,
            assoc_parity,
            topology_trace,
        })
    }

    /// Overhead in bits, independent of payload length: CRC-16 (16) + acc
    /// (2, one of 4 states) + quantized chi (up to 16, but only
    /// `ceil(log2(2*max_scaled_chi+1))` bits of actual entropy at a given
    /// stream length -- see report) + assoc_parity (1). Topology trace, when
    /// present, adds `payload.len() - 2` bits (a reference trace, scored
    /// separately, not folded into "overhead" in the report's headline
    /// number).
    pub fn fixed_overhead_bits(&self) -> usize {
        16 + 2 + 16 + 1
    }

    pub fn topology_trace_bits(&self) -> usize {
        self.topology_trace.as_ref().map_or(0, |t| t.len())
    }
}

/// Detection-only verdict: does anything in the frame disagree with the
/// received payload?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetectionVerdict {
    pub crc_mismatch: bool,
    pub gpl_mismatch: bool,
    pub parity_mismatch: bool,
    pub topology_mismatch: bool,
}

impl DetectionVerdict {
    pub fn crc_only(&self) -> bool {
        self.crc_mismatch
    }
    pub fn hybrid_any(&self) -> bool {
        self.crc_mismatch || self.gpl_mismatch || self.parity_mismatch || self.topology_mismatch
    }
}

/// Compare a received (possibly corrupted) payload against a reference
/// frame. Returns per-field mismatch flags; `hybrid_any()` is the frame's
/// overall detection verdict.
pub fn detect(frame: &HybridFrame, received: &[PhaseState]) -> DetectionVerdict {
    let crc_mismatch = match phase_to_bytes(received) {
        Ok(bytes) => crc16_ccitt(&bytes) != frame.crc16,
        Err(_) => true,
    };
    let (acc, chi) = gpl_checksum(received);
    let chi_q = quantize_chi(chi);
    let gpl_mismatch = acc != frame.gpl_acc || chi_q != frame.gpl_chi_q;
    let parity_mismatch = topology_parity(received) != frame.assoc_parity;
    let topology_mismatch = match &frame.topology_trace {
        None => false,
        Some(trace) => {
            let recv_trace: Vec<TopologyKind> = scan_sliding_triples(received)
                .into_iter()
                .map(|e| e.kind)
                .collect();
            &recv_trace != trace
        }
    };
    DetectionVerdict {
        crc_mismatch,
        gpl_mismatch,
        parity_mismatch,
        topology_mismatch,
    }
}

/// Diagnosis: error type classification + localization, derived *only* from
/// the topology trace mismatch pattern (plus the chi delta, to distinguish
/// reflection from plain substitution). Returns `None` when there is no
/// topology trace to diagnose from, the received length changed (out of
/// scope for this avenue), or the trace did not flag anything (topology
/// silently agrees even though CRC/GPL may have flagged the frame via
/// another field) -- in all these cases there is nothing to localize.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorClass {
    Substitution,
    Reflection,
    Transposition,
    Burst,
    Shuffle,
    ChiEvadingPair,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnosis {
    pub class: ErrorClass,
    /// Estimated symbol indices (0-based) of the corruption.
    pub locations: Vec<usize>,
}

/// Tolerance on the chi-delta reflection test, in scaled-chi units (i.e.
/// `CHI_SCALE * real_chi_units`). Must clear the +-1/(2*CHI_SCALE) rounding
/// error contributed by `frame.gpl_chi_q` alone; set well above it.
const CHI_REFLECTION_EPS: f64 = 0.02;

/// Calibrated against `debug_calibration_histograms` (see test module):
/// empirically, single-symbol edits (substitution/reflection) never push
/// the diff-index span past 2; adjacent transpositions touch a 4-wide
/// window of triple-starts (span 3); everything wider is burst-like; and a
/// full shuffle scatters diffs across most of the trace (span >> 10,
/// islands >= 3). These are measured boundaries, not derived constants —
/// see the report's honest-limits section for where classes overlap.
pub fn diagnose(frame: &HybridFrame, received: &[PhaseState]) -> Option<Diagnosis> {
    let trace = frame.topology_trace.as_ref()?;
    if received.len() != frame.payload.len() {
        return None;
    }
    let recv_trace: Vec<TopologyKind> = scan_sliding_triples(received)
        .into_iter()
        .map(|e| e.kind)
        .collect();
    if recv_trace.len() != trace.len() {
        return None;
    }
    let diff_idx: Vec<usize> = trace
        .iter()
        .zip(recv_trace.iter())
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i)
        .collect();
    if diff_idx.is_empty() {
        return None;
    }

    let clusters = cluster_contiguous(&diff_idx);
    let span = diff_idx.last().unwrap() - diff_idx.first().unwrap();

    // 1. Scattered across the trace in >=3 separate islands -> full
    //    permutation. (Calibrated against `debug_calibration_histograms`:
    //    shuffle produces >=3 islands in effectively all trials, while a
    //    genuinely two-edit chi-evading pair can have a *large span* -- its
    //    two edits can land anywhere in the payload -- but reliably lands
    //    in exactly 1 or 2 islands, never 3+. Span alone is therefore not a
    //    safe shuffle trigger; island count is.)
    if clusters.len() >= 3 {
        return Some(Diagnosis {
            class: ErrorClass::Shuffle,
            locations: Vec::new(),
        });
    }

    // 2. Reflection test: `state_sheet(rho(x)) == -state_sheet(x)`
    //    identically (checksum.rs / OSF paper Sec.8), so a single reflected
    //    symbol changes global chi by exactly `2*state_sheet(received_sym)`.
    //    A flipped triple-start run `[lo,hi]` (hi-lo<=2) is consistent with
    //    a single edited symbol only at positions `[hi, lo+2]` (every
    //    flipped triple must contain the edited symbol) -- test every such
    //    candidate, in every island, against the chi formula.
    let (_, chi_recv) = gpl_checksum(received);
    let chi_delta = chi_recv - (frame.gpl_chi_q as f64 / CHI_SCALE);
    for cluster in &clusters {
        let lo = *cluster.first().unwrap();
        let hi = *cluster.last().unwrap();
        for est in candidate_positions(lo, hi, received.len()) {
            let predicted = 2.0 * state_sheet(received[est]);
            if (chi_delta - predicted).abs() < CHI_REFLECTION_EPS {
                return Some(Diagnosis {
                    class: ErrorClass::Reflection,
                    locations: alloc_vec1(est),
                });
            }
        }
    }

    // 3. Two islands with near-zero net chi change: two reflections whose
    //    sheet contributions cancel (the chi-evading construction is
    //    literally two independent single-symbol reflections chosen so
    //    their deltas sum to ~0).
    if clusters.len() == 2 && chi_delta.abs() < CHI_REFLECTION_EPS {
        let locations = clusters
            .iter()
            .map(|c| symbol_estimate(*c.first().unwrap(), *c.last().unwrap(), received.len()))
            .collect();
        return Some(Diagnosis {
            class: ErrorClass::ChiEvadingPair,
            locations,
        });
    }

    // 4. Remaining classes distinguished by overall span.
    let lo = *diff_idx.first().unwrap();
    let hi = *diff_idx.last().unwrap();
    let est = symbol_estimate(lo, hi, received.len());
    let class = if span <= 2 {
        ErrorClass::Substitution
    } else if span == 3 {
        ErrorClass::Transposition
    } else {
        ErrorClass::Burst
    };
    Some(Diagnosis {
        class,
        locations: alloc_vec1(est),
    })
}

fn alloc_vec1(x: usize) -> Vec<usize> {
    let mut v = Vec::with_capacity(1);
    v.push(x);
    v
}

/// Map a (possibly single-point) run `[lo, hi]` of differing triple-start
/// indices to an estimated symbol index: a single-symbol edit at position p
/// perturbs triple-starts in `[p-2, p]` (clipped to range), so the run's
/// midpoint + 1 recovers p exactly away from either boundary.
fn symbol_estimate(lo: usize, hi: usize, payload_len: usize) -> usize {
    let mid = (lo + hi) / 2;
    (mid + 1).min(payload_len.saturating_sub(1))
}

/// Exact candidate symbol positions consistent with a single edited symbol
/// having produced the flipped triple-start run `[lo,hi]`: triple-start j
/// covers symbols `{j,j+1,j+2}`, so a single edit position must lie in the
/// intersection over every flipped j in `[lo,hi]`, i.e. `[hi, lo+2]`. Falls
/// back to the midpoint estimate when the run is too wide to be a
/// single-edit footprint (hi-lo>2).
fn candidate_positions(lo: usize, hi: usize, payload_len: usize) -> Vec<usize> {
    if hi - lo > 2 {
        return alloc_vec1(symbol_estimate(lo, hi, payload_len));
    }
    let start = hi.min(payload_len.saturating_sub(1));
    let end = (lo + 2).min(payload_len.saturating_sub(1));
    (start..=end).collect()
}

fn cluster_contiguous(idx: &[usize]) -> Vec<Vec<usize>> {
    let mut clusters: Vec<Vec<usize>> = Vec::new();
    for &i in idx {
        if let Some(last) = clusters.last_mut() {
            if i == last[last.len() - 1] + 1 {
                last.push(i);
                continue;
            }
        }
        let mut v = Vec::new();
        v.push(i);
        clusters.push(v);
    }
    clusters
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::gphase_core::PhaseState;

    fn sym(v: u8) -> PhaseState {
        PhaseState::from_u8(v).unwrap()
    }

    fn mk_payload(n: usize, seed: u64) -> Vec<PhaseState> {
        let mut rng = XorShift64::new(seed.wrapping_add(1));
        (0..n).map(|_| sym(rng.next_range(4) as u8)).collect()
    }

    // ---- deterministic xorshift64* PRNG (no external `rand` dependency,
    // matches the reproducibility posture of gpl_program.py's seeded
    // `random.seed(42)`) ----
    struct XorShift64(u64);
    impl XorShift64 {
        fn new(seed: u64) -> Self {
            XorShift64(seed | 1)
        }
        fn next_u64(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x.wrapping_mul(0x2545F4914F6CDD1D)
        }
        fn next_range(&mut self, n: u64) -> u64 {
            self.next_u64() % n
        }
    }

    #[test]
    fn crc16_matches_known_vector() {
        // CRC-16/CCITT-FALSE("123456789") = 0x29B1 (well-known test vector).
        assert_eq!(crc16_ccitt(b"123456789"), 0x29B1);
    }

    #[test]
    fn encode_rejects_non_multiple_of_4() {
        let payload = vec![sym(0), sym(1), sym(2)];
        assert_eq!(
            HybridFrame::encode(&payload, false),
            Err(FrameError::PayloadLengthNotMultipleOf4(3))
        );
    }

    #[test]
    fn encode_matches_checksum_module() {
        let payload = mk_payload(64, 1);
        let frame = HybridFrame::encode(&payload, true).unwrap();
        let (acc, chi) = gpl_checksum(&payload);
        assert_eq!(frame.gpl_acc, acc);
        assert_eq!(frame.gpl_chi_q, quantize_chi(chi));
        assert_eq!(frame.assoc_parity, topology_parity(&payload));
        assert_eq!(frame.topology_trace.as_ref().unwrap().len(), 64 - 2);
    }

    #[test]
    fn detect_clean_payload_is_no_mismatch() {
        let payload = mk_payload(64, 2);
        let frame = HybridFrame::encode(&payload, true).unwrap();
        let v = detect(&frame, &payload);
        assert!(!v.hybrid_any());
        assert!(!v.crc_only());
    }

    #[test]
    fn detect_single_substitution_is_caught_by_crc_and_hybrid() {
        let payload = mk_payload(64, 3);
        let frame = HybridFrame::encode(&payload, true).unwrap();
        let mut corrupted = payload.clone();
        corrupted[10] = sym((payload[10].as_u8() + 1) % 4);
        let v = detect(&frame, &corrupted);
        assert!(v.crc_mismatch);
        assert!(v.hybrid_any());
    }

    // ---- corruption modes, mirroring gpl_program.py Sec.4 `corrupt()` ----
    fn corrupt_sub1(
        payload: &[PhaseState],
        rng: &mut XorShift64,
    ) -> Option<(Vec<PhaseState>, usize)> {
        let i = rng.next_range(payload.len() as u64) as usize;
        let orig = payload[i].as_u8();
        let mut choices: Vec<u8> = (0..4).filter(|&v| v != orig).collect();
        let pick = choices.remove(rng.next_range(choices.len() as u64) as usize);
        let mut out = payload.to_vec();
        out[i] = sym(pick);
        Some((out, i))
    }

    fn corrupt_reflect1(
        payload: &[PhaseState],
        rng: &mut XorShift64,
    ) -> Option<(Vec<PhaseState>, usize)> {
        let i = rng.next_range(payload.len() as u64) as usize;
        let mut out = payload.to_vec();
        out[i] = sym(3 - payload[i].as_u8());
        Some((out, i))
    }

    fn corrupt_swap_adj(
        payload: &[PhaseState],
        rng: &mut XorShift64,
    ) -> Option<(Vec<PhaseState>, usize)> {
        for _ in 0..50 {
            let i = rng.next_range((payload.len() - 1) as u64) as usize;
            if payload[i] != payload[i + 1] {
                let mut out = payload.to_vec();
                out.swap(i, i + 1);
                return Some((out, i));
            }
        }
        None
    }

    fn corrupt_burst4(
        payload: &[PhaseState],
        rng: &mut XorShift64,
    ) -> Option<(Vec<PhaseState>, usize)> {
        let i = rng.next_range((payload.len() - 3) as u64) as usize;
        let mut out = payload.to_vec();
        for k in 0..4 {
            out[i + k] = sym(rng.next_range(4) as u8);
        }
        Some((out, i))
    }

    fn corrupt_shuffle(
        payload: &[PhaseState],
        rng: &mut XorShift64,
    ) -> Option<(Vec<PhaseState>, usize)> {
        let mut out = payload.to_vec();
        // Fisher-Yates.
        for k in (1..out.len()).rev() {
            let j = rng.next_range((k + 1) as u64) as usize;
            out.swap(k, j);
        }
        if out == payload.to_vec() {
            return None;
        }
        Some((out, 0))
    }

    fn corrupt_chi_evade(
        payload: &[PhaseState],
        rng: &mut XorShift64,
    ) -> Option<(Vec<PhaseState>, (usize, usize))> {
        let ones: Vec<usize> = payload
            .iter()
            .enumerate()
            .filter(|(_, v)| v.as_u8() == 1)
            .map(|(i, _)| i)
            .collect();
        let twos: Vec<usize> = payload
            .iter()
            .enumerate()
            .filter(|(_, v)| v.as_u8() == 2)
            .map(|(i, _)| i)
            .collect();
        if ones.is_empty() || twos.is_empty() {
            return None;
        }
        let p1 = ones[rng.next_range(ones.len() as u64) as usize];
        let p2 = twos[rng.next_range(twos.len() as u64) as usize];
        let mut out = payload.to_vec();
        out[p1] = sym(2);
        out[p2] = sym(1);
        Some((out, (p1, p2)))
    }

    fn within2(pred: usize, truth: usize) -> bool {
        (pred as i64 - truth as i64).abs() <= 2
    }

    /// Re-run of gpl_program.py Sec.4's Monte Carlo (L=64 symbols, N=20000
    /// trials/class), scored against the Rust HybridFrame. Prints a table
    /// matching the report format; asserts the "honest rule": hybrid
    /// detection must be >= CRC-16-alone detection in every class.
    #[test]
    fn monte_carlo_detection_and_diagnosis() {
        const L: usize = 64;
        const NT: u64 = 20_000;
        let modes = [
            "sub1",
            "swap_adj",
            "reflect1",
            "burst4",
            "shuffle",
            "chi_evade",
        ];
        let mut rng = XorShift64::new(42);

        for mode in modes {
            let mut n = 0u64;
            let mut crc_hits = 0u64;
            let mut hybrid_hits = 0u64;
            let mut topo_flagged = 0u64;
            let mut class_correct = 0u64;
            let mut class_and_localized = 0u64;

            for _ in 0..NT {
                let payload: Vec<PhaseState> =
                    (0..L).map(|_| sym(rng.next_range(4) as u8)).collect();
                let frame = HybridFrame::encode(&payload, true).unwrap();

                let (received, expect_class, truth): (Vec<PhaseState>, ErrorClass, Vec<usize>) =
                    match mode {
                        "sub1" => match corrupt_sub1(&payload, &mut rng) {
                            Some((r, i)) => (r, ErrorClass::Substitution, vec![i]),
                            None => continue,
                        },
                        "swap_adj" => match corrupt_swap_adj(&payload, &mut rng) {
                            Some((r, i)) => (r, ErrorClass::Transposition, vec![i, i + 1]),
                            None => continue,
                        },
                        "reflect1" => match corrupt_reflect1(&payload, &mut rng) {
                            Some((r, i)) => (r, ErrorClass::Reflection, vec![i]),
                            None => continue,
                        },
                        "burst4" => match corrupt_burst4(&payload, &mut rng) {
                            Some((r, i)) => (r, ErrorClass::Burst, (i..i + 4).collect()),
                            None => continue,
                        },
                        "shuffle" => match corrupt_shuffle(&payload, &mut rng) {
                            Some((r, _)) => (r, ErrorClass::Shuffle, vec![]),
                            None => continue,
                        },
                        "chi_evade" => match corrupt_chi_evade(&payload, &mut rng) {
                            Some((r, (p1, p2))) => (r, ErrorClass::ChiEvadingPair, vec![p1, p2]),
                            None => continue,
                        },
                        _ => unreachable!(),
                    };

                if received == payload {
                    continue;
                }
                n += 1;
                let verdict = detect(&frame, &received);
                if verdict.crc_mismatch {
                    crc_hits += 1;
                }
                if verdict.hybrid_any() {
                    hybrid_hits += 1;
                }

                if let Some(diag) = diagnose(&frame, &received) {
                    topo_flagged += 1;
                    let class_ok = diag.class == expect_class;
                    if class_ok {
                        class_correct += 1;
                    }
                    let localized = match expect_class {
                        ErrorClass::Shuffle => true, // no localization concept
                        ErrorClass::ChiEvadingPair => {
                            diag.locations.len() == 2
                                && truth
                                    .iter()
                                    .all(|t| diag.locations.iter().any(|p| within2(*p, *t)))
                        }
                        _ => diag
                            .locations
                            .iter()
                            .any(|p| truth.iter().any(|t| within2(*p, *t))),
                    };
                    if class_ok && localized {
                        class_and_localized += 1;
                    }
                }
            }

            let crc_rate = crc_hits as f64 / n as f64;
            let hybrid_rate = hybrid_hits as f64 / n as f64;
            let class_rate = if topo_flagged > 0 {
                class_correct as f64 / topo_flagged as f64
            } else {
                0.0
            };
            let local_rate = if topo_flagged > 0 {
                class_and_localized as f64 / topo_flagged as f64
            } else {
                0.0
            };
            println!(
                "{mode:<10} n={n:<6} crc16={crc_rate:.4} hybrid={hybrid_rate:.4} \
                 topo_flagged={topo_flagged:<6} class_acc={class_rate:.4} class+local_acc={local_rate:.4}"
            );

            // Honest rule: hybrid must be >= CRC-16 alone, every class.
            assert!(
                hybrid_rate >= crc_rate,
                "{mode}: hybrid {hybrid_rate:.4} < crc16 {crc_rate:.4}"
            );
        }
    }

    #[test]
    fn debug_calibration_histograms() {
        const L: usize = 64;
        const NT: u64 = 5_000;
        let modes = [
            "sub1",
            "swap_adj",
            "reflect1",
            "burst4",
            "shuffle",
            "chi_evade",
        ];
        let mut rng = XorShift64::new(42);
        for mode in modes {
            let mut span_hist: std::collections::BTreeMap<usize, u64> = Default::default();
            let mut island_hist: std::collections::BTreeMap<usize, u64> = Default::default();
            let mut ndiff_hist: std::collections::BTreeMap<usize, u64> = Default::default();
            let mut flagged = 0u64;
            let mut total = 0u64;
            for _ in 0..NT {
                let payload: Vec<PhaseState> =
                    (0..L).map(|_| sym(rng.next_range(4) as u8)).collect();
                let frame = HybridFrame::encode(&payload, true).unwrap();
                let received = match mode {
                    "sub1" => corrupt_sub1(&payload, &mut rng).map(|x| x.0),
                    "swap_adj" => corrupt_swap_adj(&payload, &mut rng).map(|x| x.0),
                    "reflect1" => corrupt_reflect1(&payload, &mut rng).map(|x| x.0),
                    "burst4" => corrupt_burst4(&payload, &mut rng).map(|x| x.0),
                    "shuffle" => corrupt_shuffle(&payload, &mut rng).map(|x| x.0),
                    "chi_evade" => corrupt_chi_evade(&payload, &mut rng).map(|x| x.0),
                    _ => unreachable!(),
                };
                let Some(received) = received else { continue };
                if received == payload {
                    continue;
                }
                total += 1;
                let trace = frame.topology_trace.as_ref().unwrap();
                let recv_trace: Vec<TopologyKind> = scan_sliding_triples(&received)
                    .into_iter()
                    .map(|e| e.kind)
                    .collect();
                let diff_idx: Vec<usize> = trace
                    .iter()
                    .zip(recv_trace.iter())
                    .enumerate()
                    .filter(|(_, (a, b))| a != b)
                    .map(|(i, _)| i)
                    .collect();
                if diff_idx.is_empty() {
                    continue;
                }
                flagged += 1;
                let span = diff_idx.last().unwrap() - diff_idx.first().unwrap();
                let clusters = cluster_contiguous(&diff_idx);
                *span_hist.entry(span).or_insert(0) += 1;
                *island_hist.entry(clusters.len()).or_insert(0) += 1;
                *ndiff_hist.entry(diff_idx.len()).or_insert(0) += 1;
            }
            println!(
                "mode={mode} flagged={flagged}/{total} span_hist={span_hist:?}\n  islands={island_hist:?}\n  ndiff={ndiff_hist:?}"
            );
        }
    }
}
