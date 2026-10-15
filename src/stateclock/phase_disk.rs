//! PHASEGUARD phase-disk parallel solver — research, feature-gated.
//!
//! Docket CAREGAGA-GPOS-2026-001, **Gate 1** host math reference model. This is
//! HYPOTHESIS-stage falsification research, NOT a proven feature. Nothing here
//! may be described as an implemented kernel-integrity mechanism until the gate
//! ladder passes.
//!
//! ## What this module is (and is not)
//!
//! It is a **separate, parameterized** solver for the signed-branch biquadratic
//!
//! ```text
//! y^4 - phi*y^2 + sigma*alpha = 0,   sigma in {+1, -1}
//! ```
//!
//! and the product-invariant squeeze/release family
//!
//! ```text
//! A(s) = alpha^(1/4) * e^{+s},   B(s) = alpha^(1/4) * e^{-s},   A(s)*B(s) = sqrt(alpha).
//! ```
//!
//! It **never** feeds [`crate::core::gphase_core`]'s canonical `params()`.
//! `gphase_core` stays the sole algebra authority: `sigma = +1` here reproduces
//! the canonical roots as a *cross-check only*, and any consumer that needs the
//! real algebra must call `gphase_core`, not this module.
//!
//! Per the plan, this Gate-1 reference model uses `f64`. Kernel integration
//! (Gate 4) will use the deterministic fixed-point pattern from
//! `protocol::hybrid_frame`; that is out of scope here.

use crate::core::gphase_core::{phi, ALPHA};
use crate::platform::Vec;

// ── float shims (std intrinsic / libm on bare metal), matching
//    `core::gphase_core::fsqrt` so this module is baremetal-ready later. ──
#[inline]
fn fsqrt(x: f64) -> f64 {
    #[cfg(feature = "std")]
    {
        x.sqrt()
    }
    #[cfg(all(feature = "baremetal", not(feature = "std")))]
    {
        libm::sqrt(x)
    }
}

#[inline]
fn fexp(x: f64) -> f64 {
    #[cfg(feature = "std")]
    {
        x.exp()
    }
    #[cfg(all(feature = "baremetal", not(feature = "std")))]
    {
        libm::exp(x)
    }
}

#[inline]
fn fln(x: f64) -> f64 {
    #[cfg(feature = "std")]
    {
        x.ln()
    }
    #[cfg(all(feature = "baremetal", not(feature = "std")))]
    {
        libm::log(x)
    }
}

#[inline]
fn fabs(x: f64) -> f64 {
    #[cfg(feature = "std")]
    {
        x.abs()
    }
    #[cfg(all(feature = "baremetal", not(feature = "std")))]
    {
        libm::fabs(x)
    }
}

#[inline]
fn fcos(x: f64) -> f64 {
    #[cfg(feature = "std")]
    {
        x.cos()
    }
    #[cfg(all(feature = "baremetal", not(feature = "std")))]
    {
        libm::cos(x)
    }
}

/// The sign in `y^4 - phi*y^2 + sigma*alpha = 0`.
///
/// `Positive` (sigma = +1) is the canonical branch and must reproduce
/// `gphase_core`. `Negative` (sigma = -1) is the research branch on which the
/// inner root pair becomes imaginary (`+/- i*b`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignedBranch {
    Positive,
    Negative,
}

impl SignedBranch {
    #[inline]
    pub fn sigma(self) -> f64 {
        match self {
            SignedBranch::Positive => 1.0,
            SignedBranch::Negative => -1.0,
        }
    }
}

/// Roots of `y^4 - phi*y^2 + sigma*alpha = 0` for one branch.
///
/// The outer pair is always real. On the `Negative` branch the inner squared
/// root `y2_inner` is negative, so the inner pair is imaginary `+/- i*b` with
/// `b = sqrt(-y2_inner)`; `inner_real` records which case holds.
#[derive(Debug, Clone, Copy)]
pub struct BiquadRoots {
    pub sigma: f64,
    pub discriminant: f64,
    pub y2_outer: f64,
    pub y2_inner: f64,
    /// Real magnitude of the outer pair (`+/- outer`).
    pub outer: f64,
    /// `true`: inner pair is real (`+/- inner_mag`).
    /// `false`: inner pair is imaginary (`+/- i * inner_mag`).
    pub inner_real: bool,
    /// `sqrt(|y2_inner|)`: real inner magnitude, or `|b|` for the `i*b` pair.
    pub inner_mag: f64,
}

/// Solve the signed-branch biquadratic. Mirrors the *shape* of
/// `gphase_core::compute_params` but is parameterized on `sigma` and never
/// writes canonical state.
pub fn biquadratic_roots(branch: SignedBranch) -> BiquadRoots {
    let p = phi();
    let sigma = branch.sigma();
    // discriminant = phi^2 - 4*sigma*alpha
    let discriminant = p * p - 4.0 * sigma * ALPHA;
    let sqrt_disc = fsqrt(discriminant);
    let y2_outer = (p + sqrt_disc) / 2.0;
    let y2_inner = (p - sqrt_disc) / 2.0;

    let outer = fsqrt(y2_outer);
    let inner_real = y2_inner >= 0.0;
    let inner_mag = fsqrt(fabs(y2_inner));

    BiquadRoots {
        sigma,
        discriminant,
        y2_outer,
        y2_inner,
        outer,
        inner_real,
        inner_mag,
    }
}

impl BiquadRoots {
    /// `|y_outer^2 + y_inner^2 - phi|`. For the canonical branch this is the
    /// sum invariant and should be ~0; the value uses the *signed* `y2_inner`,
    /// so on the imaginary branch it measures the real-part sum.
    pub fn sum_residual(&self) -> f64 {
        fabs(self.y2_outer + self.y2_inner - phi())
    }

    /// `|y_outer * y_inner_magnitude - sqrt(sigma*alpha)|` is only meaningful on
    /// the real branch; here we report `|y2_outer * y2_inner - sigma*alpha|`,
    /// the exact product of squared roots (Vieta), valid on both branches.
    pub fn product_residual(&self) -> f64 {
        fabs(self.y2_outer * self.y2_inner - self.sigma * ALPHA)
    }
}

/// A point on the product-invariant squeeze/release family.
///
/// `A(s) = alpha^(1/4) e^{+s}`, `B(s) = alpha^(1/4) e^{-s}`. The product
/// `A*B = sqrt(alpha)` is conserved for every `s`; `s` is the pure squeeze
/// coordinate (the log half-ratio `s = (1/2) ln(A/B)`).
#[derive(Debug, Clone, Copy)]
pub struct SqueezeState {
    pub s: f64,
    pub a: f64,
    pub b: f64,
}

/// `alpha^(1/4)` — the isotropic point where `A = B`.
#[inline]
pub fn squeeze_scale() -> f64 {
    fsqrt(fsqrt(ALPHA))
}

/// Evaluate the squeeze family at coordinate `s`.
pub fn squeeze_at(s: f64) -> SqueezeState {
    let scale = squeeze_scale();
    SqueezeState {
        s,
        a: scale * fexp(s),
        b: scale * fexp(-s),
    }
}

impl SqueezeState {
    /// `|A*B - sqrt(alpha)|` — the conserved product; ~0 for every `s`.
    pub fn product_residual(&self) -> f64 {
        fabs(self.a * self.b - fsqrt(ALPHA))
    }
}

/// The squeeze coordinate `s` at which the product-invariant family coincides
/// with the canonical operating point: `s = (1/4) ln(y2_outer / y2_inner)` on
/// the `Positive` branch. This is the mathematical link between the squeeze
/// family and the canonical biquadratic (the sum constraint selects this `s`).
pub fn canonical_squeeze_coordinate() -> f64 {
    let r = biquadratic_roots(SignedBranch::Positive);
    0.25 * fln(r.y2_outer / r.y2_inner)
}

/// Radial motion classification from a squeeze-coordinate delta `ds`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadialDirection {
    Squeezing,
    Releasing,
    Hold,
}

/// Classify `ds/dt` sign (with a dead-band `eps` to avoid dithering).
pub fn radial_direction(ds: f64, eps: f64) -> RadialDirection {
    if ds > eps {
        RadialDirection::Squeezing
    } else if ds < -eps {
        RadialDirection::Releasing
    } else {
        RadialDirection::Hold
    }
}

/// One row of a squeeze-cycle trace (Gate-1 evidence generator).
#[derive(Debug, Clone, Copy)]
pub struct SqueezeTraceRow {
    pub step: usize,
    pub s: f64,
    pub a: f64,
    pub b: f64,
    pub product_residual: f64,
    pub direction: RadialDirection,
}

/// Generate a symmetric squeeze cycle `s: 0 -> +s_max -> -s_max -> 0` in
/// `steps` increments, recording the conserved product and the direction bit.
/// Pure host reference model; deterministic.
pub fn squeeze_cycle_trace(s_max: f64, steps: usize) -> Vec<SqueezeTraceRow> {
    let mut out = Vec::with_capacity(steps + 1);
    if steps == 0 {
        return out;
    }
    // Triangle wave over [0, 1) phase → s in [-s_max, +s_max], starting at 0.
    let mut prev_s = 0.0f64;
    for step in 0..=steps {
        let phase = step as f64 / steps as f64; // 0..1
                                                // triangle: 0 -> +1 -> 0 -> -1 -> 0 across the full cycle
        let tri = {
            let t = phase * 4.0;
            if t < 1.0 {
                t
            } else if t < 3.0 {
                2.0 - t
            } else {
                t - 4.0
            }
        };
        let s = s_max * tri;
        let st = squeeze_at(s);
        let ds = s - prev_s;
        let dir = radial_direction(ds, 1e-15);
        out.push(SqueezeTraceRow {
            step,
            s,
            a: st.a,
            b: st.b,
            product_residual: st.product_residual(),
            direction: dir,
        });
        prev_s = s;
    }
    out
}

// ─────────────────────────────────────────────────────────────────────────
// Gate 2 — canonical 16/4/64 census vs. phase-disk branch structure.
//
// The question: do the phase-disk features (inner/ib involvement, derived from
// the imaginary-branch inner pair {Phi1,Phi2}) EXPLAIN which 12 of the 64
// associator triples are reflections, or only reproduce the census?
//
// The census itself is taken bit-exact from `gphase_core::topology_status` (the
// authority) — never recomputed here. `threshold_energy` already encodes the
// inner (Phi1,Phi2 -> 0) vs real-rail (Phi0,Phi3 -> 1) split, i.e. exactly the
// phase-disk branch distinction.
// ─────────────────────────────────────────────────────────────────────────

use crate::core::gphase_core::{threshold_energy, topology_status, PhaseState, TopologyKind};

/// Classification of one associator triple `(a,b,c)`.
#[derive(Debug, Clone, Copy)]
pub struct TripleClass {
    pub a: PhaseState,
    pub b: PhaseState,
    pub c: PhaseState,
    /// From `gphase_core::topology_status`: is this triple a Reflection?
    pub reflection: bool,
    /// Number of inner states {Phi1,Phi2} among (a,b,c) — "ib involvement".
    pub inner_count: u8,
    /// Is the middle element `b` an inner state?
    pub middle_inner: bool,
}

/// `true` iff `s` is an inner (imaginary-branch) state Phi1/Phi2.
#[inline]
fn is_inner(s: PhaseState) -> bool {
    threshold_energy(s) == 0
}

/// Classify all 64 triples using the canonical census as ground truth.
pub fn classify_all_triples() -> Vec<TripleClass> {
    let mut out = Vec::with_capacity(64);
    for a in PhaseState::ALL {
        for b in PhaseState::ALL {
            for c in PhaseState::ALL {
                let reflection = matches!(topology_status(a, b, c).kind, TopologyKind::Reflection);
                let inner_count = is_inner(a) as u8 + is_inner(b) as u8 + is_inner(c) as u8;
                out.push(TripleClass {
                    a,
                    b,
                    c,
                    reflection,
                    inner_count,
                    middle_inner: is_inner(b),
                });
            }
        }
    }
    out
}

/// Contingency of the census against phase-disk inner-structure features.
#[derive(Debug, Clone, Copy, Default)]
pub struct CensusContingency {
    pub same: u32,
    pub reflection: u32,
    /// reflection / same counts indexed by inner_count 0..=3.
    pub refl_by_inner_count: [u32; 4],
    pub same_by_inner_count: [u32; 4],
    /// middle-element split.
    pub refl_middle_inner: u32,
    pub refl_middle_rail: u32,
    pub same_middle_inner: u32,
    pub same_middle_rail: u32,
}

pub fn census_contingency() -> CensusContingency {
    let mut ct = CensusContingency::default();
    for t in classify_all_triples() {
        if t.reflection {
            ct.reflection += 1;
            ct.refl_by_inner_count[t.inner_count as usize] += 1;
            if t.middle_inner {
                ct.refl_middle_inner += 1;
            } else {
                ct.refl_middle_rail += 1;
            }
        } else {
            ct.same += 1;
            ct.same_by_inner_count[t.inner_count as usize] += 1;
            if t.middle_inner {
                ct.same_middle_inner += 1;
            } else {
                ct.same_middle_rail += 1;
            }
        }
    }
    ct
}

/// Result of testing whether a boolean predicate over phase-disk features is a
/// *perfect* classifier of the reflection set.
#[derive(Debug, Clone, Copy)]
pub struct PredicateScore {
    /// predicted reflection AND is reflection.
    pub true_pos: u32,
    /// predicted reflection AND is same (predicate over-fires → not sufficient).
    pub false_pos: u32,
    /// predicted same AND is reflection (predicate misses → not necessary).
    pub false_neg: u32,
    pub true_neg: u32,
}

impl PredicateScore {
    pub fn perfect(&self) -> bool {
        self.false_pos == 0 && self.false_neg == 0
    }
    /// Predicate never misses a reflection (necessary condition holds).
    pub fn necessary(&self) -> bool {
        self.false_neg == 0
    }
    /// Predicate never over-fires (sufficient condition holds).
    pub fn sufficient(&self) -> bool {
        self.false_pos == 0
    }
}

/// Score an arbitrary phase-disk-feature predicate against the census.
pub fn score_predicate(pred: impl Fn(&TripleClass) -> bool) -> PredicateScore {
    let mut s = PredicateScore {
        true_pos: 0,
        false_pos: 0,
        false_neg: 0,
        true_neg: 0,
    };
    for t in classify_all_triples() {
        match (pred(&t), t.reflection) {
            (true, true) => s.true_pos += 1,
            (true, false) => s.false_pos += 1,
            (false, true) => s.false_neg += 1,
            (false, false) => s.true_neg += 1,
        }
    }
    s
}

// ─────────────────────────────────────────────────────────────────────────
// Gate 3 — host fault injection and read-only detection.
//
// A "phase-disk scan" is modelled as a sequence of observations, each carrying
// the phase-disk state (sector, squeeze coordinate, branch, sequence tick). A
// healthy scan advances sectors monotonically mod SECTOR_COUNT, increments the
// sequence, keeps the branch Positive, and keeps A(s)*B(s)=sqrt(alpha).
//
// The Mode-A witness is READ-ONLY: it checks structural invariants and reports
// the first fault it can name. The point of the gate is to measure BOTH the
// detected and the UNDETECTED fault classes honestly.
// ─────────────────────────────────────────────────────────────────────────

use crate::stateclock::geometry::SECTOR_COUNT;

/// One observation in a phase-disk scan (Gate-3 host model).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhaseObservation {
    pub sequence: u64,
    pub sector: u8,
    pub s: f64,
    pub branch: SignedBranch,
}

/// Fault taxonomy injected at Gate 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InjectedFault {
    None,
    Skipped,
    Duplicated,
    Reversed,
    Stale,
    Replay,
    Stall,
    WrongSqueeze,
    /// Squeeze coordinate replaced by a *plausible finite* wrong value. Because
    /// `A(s)*B(s)=sqrt(alpha)` for every `s`, this preserves the product
    /// invariant and is a deliberate blind-spot probe.
    SqueezeFalsified,
    BranchFlip,
    Corruption,
}

/// What the read-only witness concluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WitnessVerdict {
    Ok,
    SequenceRegressed,
    SequenceStalled,
    SectorDiscontinuity,
    SqueezeInvariantBroken,
    BranchFlipped,
    /// Gate 3B: observed squeeze value disagrees with the clock-derived
    /// causal prediction (falsified location on the product manifold).
    SqueezePredictionMismatch,
}

/// Angular position on the phase disk for a given dual-clock tick. One full
/// sector per tick, wrapping every `SECTOR_COUNT` ticks. Parameter-free.
///
/// Equivalent form: `θ = n·(π/5)` since `2π/10 = π/5` (36° Golden Phase step).
#[inline]
fn disk_angle(sequence: u64) -> f64 {
    use core::f64::consts::PI;
    2.0 * PI * (sequence as f64) / (SECTOR_COUNT as f64)
}

/// Ten-position phase index from a **logical** StateClock tick.
///
/// ```text
/// phase_index = logical_tick mod 10
/// ```
///
/// This is the Gate-4 mapping from the authoritative kernel stateclock into the
/// Gate-3B ten-step cycle. It is **not** `StateClock.sector_id` (which comes
/// from φ geometry and advances every `DEFAULT_TICKS_PER_SECTOR` Chronos ticks).
#[inline]
pub fn phase_index_from_logical_tick(logical_tick: u64) -> u8 {
    (logical_tick % SECTOR_COUNT as u64) as u8
}

/// Angle for a phase index: `θ = phase_index · (π/5)` (36° step).
#[inline]
pub fn phase_theta_rad(phase_index: u8) -> f64 {
    use core::f64::consts::PI;
    (phase_index as f64) * (PI / 5.0)
}

/// Degrees helper for traces / QEMU evidence.
#[inline]
pub fn phase_theta_deg(phase_index: u8) -> f64 {
    (phase_index as f64) * 36.0
}

/// **Causal squeeze prediction** (Gate 3B). Given only the dual-clock position
/// (the tick / `sequence`), predict the squeeze coordinate the phase disk *must*
/// be at, from a closed-form law with **no free parameters**:
///
/// ```text
/// s_expected(tick) = s* · cos( 2π · tick / SECTOR_COUNT )
///                  = s* · cos( tick · π/5 )
/// ```
///
/// The amplitude `s*` is the canonical squeeze coordinate (Gate 1), fixed by
/// `alpha`/`phi` — not fitted. The predictor never reads the observation's own
/// `s`; it derives the expectation from information available **before** the
/// resulting state is accepted. There is no per-case lookup table.
///
/// Host Gate-3B tests use this function as the mathematical reference.
pub fn predict_squeeze(sequence: u64) -> f64 {
    canonical_squeeze_coordinate() * fcos(disk_angle(sequence))
}

/// Live-kernel form of the Gate-3B predictor: explicit `mod 10` then `π/5`.
///
/// ```text
/// phase_index = logical_tick mod 10
/// theta       = phase_index × π/5
/// expected    = s* × cos(theta)
/// ```
///
/// Must agree with [`predict_squeeze`] for the same integer tick (cos is 2π-periodic
/// and 10·(π/5)=2π). Driven only by the logical tick — never by observed squeeze.
pub fn predict_squeeze_from_logical_tick(logical_tick: u64) -> f64 {
    let idx = phase_index_from_logical_tick(logical_tick);
    canonical_squeeze_coordinate() * fcos(phase_theta_rad(idx))
}

/// Generate a healthy scan of `n` observations starting at tick `start`
/// (monotone sector + sequence, Positive branch). The squeeze value obeys the
/// causal law `predict_squeeze`, so any valid scan lies on the predicted curve.
pub fn healthy_scan_from(start: u64, n: usize) -> Vec<PhaseObservation> {
    let mut out = Vec::with_capacity(n);
    let sc = SECTOR_COUNT as u64;
    for k in 0..n as u64 {
        let seq = start + k;
        out.push(PhaseObservation {
            sequence: seq,
            sector: (seq % sc) as u8,
            s: predict_squeeze(seq),
            branch: SignedBranch::Positive,
        });
    }
    out
}

/// Healthy scan starting at tick 0.
pub fn healthy_scan(n: usize) -> Vec<PhaseObservation> {
    healthy_scan_from(0, n)
}

/// Inject a single fault at index `at` into a healthy scan (returns a new scan).
pub fn inject(scan: &[PhaseObservation], fault: InjectedFault, at: usize) -> Vec<PhaseObservation> {
    let mut v: Vec<PhaseObservation> = scan.iter().copied().collect();
    if v.is_empty() || at >= v.len() {
        return v;
    }
    match fault {
        InjectedFault::None => {}
        InjectedFault::Skipped => {
            // jump the sector forward by 2 instead of 1
            v[at].sector = ((v[at].sector as u64 + 1) % SECTOR_COUNT as u64) as u8;
        }
        InjectedFault::Duplicated => {
            if at > 0 {
                v[at].sector = v[at - 1].sector;
            }
        }
        InjectedFault::Reversed => {
            if at > 0 {
                let s = v[at].sector;
                v[at].sector = v[at - 1].sector;
                v[at - 1].sector = s;
                // also swap sequence to make it a true reversal
                let q = v[at].sequence;
                v[at].sequence = v[at - 1].sequence;
                v[at - 1].sequence = q;
            }
        }
        InjectedFault::Stale => {
            if at > 0 {
                v[at].sequence = v[at - 1].sequence; // sequence fails to advance
            }
        }
        InjectedFault::Replay => {
            if at > 1 {
                v[at] = v[at - 2]; // reinsert an older record verbatim
            }
        }
        InjectedFault::Stall => {
            // hold sector and sequence flat for a run
            if at > 0 {
                v[at].sector = v[at - 1].sector;
                v[at].sequence = v[at - 1].sequence;
            }
        }
        InjectedFault::WrongSqueeze => {
            v[at].s = f64::NAN; // corrupt squeeze so the product invariant breaks
        }
        InjectedFault::SqueezeFalsified => {
            // plausible finite wrong value: product invariant STILL holds, so a
            // product-only witness cannot see it (expected blind spot).
            v[at].s += 0.37;
        }
        InjectedFault::BranchFlip => {
            v[at].branch = SignedBranch::Negative;
        }
        InjectedFault::Corruption => {
            // flip the sector to an out-of-sequence value without touching seq
            v[at].sector = ((v[at].sector as u64 + 3) % SECTOR_COUNT as u64) as u8;
        }
    }
    v
}

/// Read-only Mode-A witness. Walks the scan, returns the first named fault.
pub fn witness(scan: &[PhaseObservation]) -> WitnessVerdict {
    let sc = SECTOR_COUNT as i64;
    for w in scan.windows(2) {
        let (prev, cur) = (w[0], w[1]);
        // 1. branch must stay Positive
        if cur.branch != SignedBranch::Positive {
            return WitnessVerdict::BranchFlipped;
        }
        // 2. squeeze product invariant (NaN/inf or |A*B - sqrt(alpha)| too big)
        let st = squeeze_at(cur.s);
        let resid = st.product_residual();
        if !(resid.is_finite()) || resid > 1e-9 {
            return WitnessVerdict::SqueezeInvariantBroken;
        }
        // 3. sequence must strictly increase
        if cur.sequence <= prev.sequence {
            if cur.sequence == prev.sequence {
                return WitnessVerdict::SequenceStalled;
            }
            return WitnessVerdict::SequenceRegressed;
        }
        // 4. sector must advance by exactly +1 (mod SECTOR_COUNT)
        let expected = (prev.sector as i64 + 1) % sc;
        if cur.sector as i64 != expected {
            return WitnessVerdict::SectorDiscontinuity;
        }
    }
    WitnessVerdict::Ok
}

/// Tolerance for the causal squeeze-prediction check.
pub const SQUEEZE_PRED_TOL: f64 = 1e-9;

/// **Gate 3B predictive witness.** Runs the full structural [`witness`] (which
/// preserves the 9/10 baseline) and, only if that is clean, adds the causal
/// squeeze-location check: every observation's `s` must match the clock-derived
/// [`predict_squeeze`] within tolerance. This closes the falsified-finite-squeeze
/// blind spot **without** peeking at the answer — the prediction comes solely
/// from the dual-clock tick, which is prior information.
pub fn witness_predictive(scan: &[PhaseObservation]) -> WitnessVerdict {
    let base = witness(scan);
    if base != WitnessVerdict::Ok {
        return base;
    }
    for obs in scan {
        let expected = predict_squeeze(obs.sequence);
        if fabs(obs.s - expected) > SQUEEZE_PRED_TOL {
            return WitnessVerdict::SqueezePredictionMismatch;
        }
    }
    WitnessVerdict::Ok
}

/// Detection outcome for one fault class.
#[derive(Debug, Clone, Copy)]
pub struct DetectionResult {
    pub fault: InjectedFault,
    pub detected: bool,
    pub verdict: WitnessVerdict,
}

/// Run every fault class through inject then the chosen witness at a fixed
/// index. `predictive = true` uses the Gate-3B [`witness_predictive`].
pub fn detection_matrix_with(scan_len: usize, at: usize, predictive: bool) -> Vec<DetectionResult> {
    let base = healthy_scan(scan_len);
    let faults = [
        InjectedFault::None,
        InjectedFault::Skipped,
        InjectedFault::Duplicated,
        InjectedFault::Reversed,
        InjectedFault::Stale,
        InjectedFault::Replay,
        InjectedFault::Stall,
        InjectedFault::WrongSqueeze,
        InjectedFault::SqueezeFalsified,
        InjectedFault::BranchFlip,
        InjectedFault::Corruption,
    ];
    let mut out = Vec::with_capacity(faults.len());
    for f in faults {
        let scan = inject(&base, f, at);
        let verdict = if predictive {
            witness_predictive(&scan)
        } else {
            witness(&scan)
        };
        let detected = match f {
            InjectedFault::None => verdict == WitnessVerdict::Ok,
            _ => verdict != WitnessVerdict::Ok,
        };
        out.push(DetectionResult {
            fault: f,
            detected,
            verdict,
        });
    }
    out
}

/// Run every fault class through inject→witness at a fixed index; report which
/// are detected and which slip through. Deterministic Gate-3 evidence.
pub fn detection_matrix(scan_len: usize, at: usize) -> Vec<DetectionResult> {
    let base = healthy_scan(scan_len);
    let faults = [
        InjectedFault::None,
        InjectedFault::Skipped,
        InjectedFault::Duplicated,
        InjectedFault::Reversed,
        InjectedFault::Stale,
        InjectedFault::Replay,
        InjectedFault::Stall,
        InjectedFault::WrongSqueeze,
        InjectedFault::SqueezeFalsified,
        InjectedFault::BranchFlip,
        InjectedFault::Corruption,
    ];
    let mut out = Vec::with_capacity(faults.len());
    for f in faults {
        let scan = inject(&base, f, at);
        let verdict = witness(&scan);
        let detected = match f {
            InjectedFault::None => verdict == WitnessVerdict::Ok, // "detected" here = correctly clean
            _ => verdict != WitnessVerdict::Ok,
        };
        out.push(DetectionResult {
            fault: f,
            detected,
            verdict,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOL: f64 = 1e-12;

    #[test]
    fn positive_branch_reproduces_canonical_invariants() {
        // sigma = +1 must satisfy the canonical sum/product invariants exactly.
        let r = biquadratic_roots(SignedBranch::Positive);
        assert!(r.inner_real, "canonical inner pair must be real");
        assert!(r.discriminant > 0.0, "phi^2 - 4*alpha must be positive");
        assert!(r.sum_residual() < TOL, "sum residual {}", r.sum_residual());
        assert!(
            r.product_residual() < TOL,
            "product residual {}",
            r.product_residual()
        );
    }

    #[test]
    fn negative_branch_makes_inner_pair_imaginary() {
        // sigma = -1: discriminant grows to phi^2 + 4*alpha, inner square goes
        // negative → imaginary inner pair (+/- i*b), outer stays real.
        let r = biquadratic_roots(SignedBranch::Negative);
        let expected_disc = phi() * phi() + 4.0 * ALPHA;
        assert!((r.discriminant - expected_disc).abs() < TOL);
        assert!(
            !r.inner_real,
            "negative branch inner pair must be imaginary"
        );
        assert!(r.y2_inner < 0.0, "inner squared root must be negative");
        assert!(r.outer > 0.0, "outer pair stays real");
        // Vieta product of squared roots equals sigma*alpha = -alpha.
        assert!(r.product_residual() < TOL);
    }

    #[test]
    fn squeeze_product_is_conserved_across_the_cycle() {
        // A(s)*B(s) = sqrt(alpha) for every s in a full squeeze cycle.
        let trace = squeeze_cycle_trace(2.0, 400);
        assert_eq!(trace.len(), 401);
        for row in &trace {
            assert!(
                row.product_residual < TOL,
                "step {} s={} residual {}",
                row.step,
                row.s,
                row.product_residual
            );
        }
    }

    #[test]
    fn canonical_squeeze_coordinate_reproduces_root_ratio() {
        // squeeze_at(s_canonical) must reproduce the canonical outer/inner roots
        // (product-invariant family ∩ sum constraint = the operating point).
        let s = canonical_squeeze_coordinate();
        let st = squeeze_at(s);
        let r = biquadratic_roots(SignedBranch::Positive);
        // A -> y_outer, B -> y_inner (both real, product sqrt(alpha)).
        assert!(
            (st.a - r.outer).abs() < 1e-9,
            "A={} y_outer={}",
            st.a,
            r.outer
        );
        assert!(
            (st.b - r.inner_mag).abs() < 1e-9,
            "B={} y_inner={}",
            st.b,
            r.inner_mag
        );
    }

    /// Gate-1 evidence generator (reproducible). Run with:
    /// `cargo test --features phaseguard-research phase_disk::tests::dump_reference -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn dump_reference() {
        let pos = biquadratic_roots(SignedBranch::Positive);
        let neg = biquadratic_roots(SignedBranch::Negative);
        println!("phi = {:.15}", phi());
        println!("alpha = {:.15}", ALPHA);
        println!("sqrt(alpha) = {:.15}", fsqrt(ALPHA));
        println!("alpha^(1/4) = {:.15}", squeeze_scale());
        println!("-- Positive branch (sigma=+1, canonical cross-check) --");
        println!("  discriminant = {:.15}", pos.discriminant);
        println!(
            "  y2_outer = {:.15}  y2_inner = {:.15}",
            pos.y2_outer, pos.y2_inner
        );
        println!(
            "  y_outer  = {:.15}  y_inner  = {:.15} (real)",
            pos.outer, pos.inner_mag
        );
        println!(
            "  sum_residual = {:.3e}  product_residual = {:.3e}",
            pos.sum_residual(),
            pos.product_residual()
        );
        println!("-- Negative branch (sigma=-1, research) --");
        println!("  discriminant = {:.15}", neg.discriminant);
        println!(
            "  y2_outer = {:.15}  y2_inner = {:.15}",
            neg.y2_outer, neg.y2_inner
        );
        println!(
            "  y_outer  = {:.15} (real)  b = {:.15} (inner = +/- i*b)",
            neg.outer, neg.inner_mag
        );
        println!(
            "  inner_real = {}  product_residual = {:.3e}",
            neg.inner_real,
            neg.product_residual()
        );
        println!("-- canonical squeeze coordinate --");
        println!("  s* = {:.15}", canonical_squeeze_coordinate());
        println!("-- squeeze cycle trace (s_max=1.0, 8 steps) --");
        for row in squeeze_cycle_trace(1.0, 8) {
            println!(
                "  step {} s={:+.6} A={:.9} B={:.9} AB-sqrt(a)={:.2e} dir={:?}",
                row.step, row.s, row.a, row.b, row.product_residual, row.direction
            );
        }
    }

    // ── Gate 2 ──────────────────────────────────────────────────────────

    #[test]
    fn census_matches_canonical_52_12_0() {
        // The phase-disk classification must reproduce gphase_core exactly.
        let ct = census_contingency();
        assert_eq!(ct.same, 52, "Same count");
        assert_eq!(ct.reflection, 12, "Reflection count");
        assert_eq!(ct.same + ct.reflection, 64, "total triples");
    }

    #[test]
    fn middle_inner_is_necessary_but_not_sufficient() {
        // Prior finding: every reflection triple has an inner middle element.
        // Test it as a predicate: necessary (no misses) yet not sufficient
        // (it also fires on some Same triples) — i.e. ib-structure predicts but
        // does not fully explain the 12.
        let s = score_predicate(|t| t.middle_inner);
        assert!(
            s.necessary(),
            "middle_inner should miss no reflection (false_neg={})",
            s.false_neg
        );
        assert!(
            !s.sufficient(),
            "middle_inner alone must NOT be a perfect classifier (false_pos={})",
            s.false_pos
        );
    }

    #[test]
    fn no_single_inner_predicate_perfectly_explains_the_12() {
        // Falsification: test the natural phase-disk predicates and confirm none
        // is a perfect classifier on its own. If one WERE perfect, that would be
        // a positive Gate-2 result and this assertion would flag it for review.
        let predicates: [(&str, fn(&TripleClass) -> bool); 4] = [
            ("middle_inner", |t| t.middle_inner),
            ("inner_count>=2", |t| t.inner_count >= 2),
            ("inner_count>=1", |t| t.inner_count >= 1),
            ("all_inner", |t| t.inner_count == 3),
        ];
        let mut any_perfect = false;
        for (_name, p) in predicates {
            if score_predicate(p).perfect() {
                any_perfect = true;
            }
        }
        assert!(
            !any_perfect,
            "a single inner-predicate perfectly classified the 12 — revisit Gate 2 verdict"
        );
    }

    #[test]
    #[ignore]
    fn dump_census() {
        let ct = census_contingency();
        println!(
            "census: Same={} Reflection={} (canonical 52/12/0)",
            ct.same, ct.reflection
        );
        println!(
            "reflection by inner_count [0,1,2,3] = {:?}",
            ct.refl_by_inner_count
        );
        println!(
            "same       by inner_count [0,1,2,3] = {:?}",
            ct.same_by_inner_count
        );
        println!(
            "reflection middle: inner={} rail={}",
            ct.refl_middle_inner, ct.refl_middle_rail
        );
        println!(
            "same       middle: inner={} rail={}",
            ct.same_middle_inner, ct.same_middle_rail
        );
        for (name, p) in [
            (
                "middle_inner",
                &(|t: &TripleClass| t.middle_inner) as &dyn Fn(&TripleClass) -> bool,
            ),
            ("inner_count>=2", &(|t: &TripleClass| t.inner_count >= 2)),
            ("inner_count>=1", &(|t: &TripleClass| t.inner_count >= 1)),
            ("all_inner", &(|t: &TripleClass| t.inner_count == 3)),
        ] {
            let s = score_predicate(p);
            println!(
                "predicate {:16} TP={:2} FP={:2} FN={:2} TN={:2}  necessary={} sufficient={} perfect={}",
                name, s.true_pos, s.false_pos, s.false_neg, s.true_neg,
                s.necessary(), s.sufficient(), s.perfect()
            );
        }
        // enumerate the 12 reflection triples with their inner signature
        println!("-- the 12 reflection triples --");
        for t in classify_all_triples().into_iter().filter(|t| t.reflection) {
            println!(
                "  ({:?},{:?},{:?}) inner_count={} middle_inner={}",
                t.a, t.b, t.c, t.inner_count, t.middle_inner
            );
        }
    }

    // ── Gate 3 ──────────────────────────────────────────────────────────

    #[test]
    fn healthy_scan_is_clean() {
        let scan = healthy_scan(40);
        assert_eq!(witness(&scan), WitnessVerdict::Ok);
    }

    #[test]
    fn structural_faults_are_detected() {
        // Every structural fault (sequence/sector/branch/NaN-squeeze) must be
        // caught by the read-only witness.
        let base = healthy_scan(40);
        let at = 20;
        for f in [
            InjectedFault::Skipped,
            InjectedFault::Duplicated,
            InjectedFault::Reversed,
            InjectedFault::Stale,
            InjectedFault::Replay,
            InjectedFault::Stall,
            InjectedFault::WrongSqueeze,
            InjectedFault::BranchFlip,
            InjectedFault::Corruption,
        ] {
            let scan = inject(&base, f, at);
            assert_ne!(witness(&scan), WitnessVerdict::Ok, "missed fault {:?}", f);
        }
    }

    #[test]
    fn falsified_squeeze_is_a_blind_spot() {
        // HONEST NEGATIVE: a plausible finite wrong squeeze value preserves the
        // product invariant, so the product-only witness canNOT detect it.
        let base = healthy_scan(40);
        let scan = inject(&base, InjectedFault::SqueezeFalsified, 20);
        assert_eq!(
            witness(&scan),
            WitnessVerdict::Ok,
            "falsified-but-plausible squeeze should slip past a product-only witness"
        );
    }

    #[test]
    fn detection_matrix_reports_one_undetected_class() {
        // The matrix must not be a tautology: exactly the SqueezeFalsified class
        // is expected to go undetected here; None is expected clean.
        let matrix = detection_matrix(40, 20);
        let undetected: Vec<_> = matrix
            .iter()
            .filter(|r| r.fault != InjectedFault::None && !r.detected)
            .map(|r| r.fault)
            .collect();
        assert_eq!(
            undetected,
            vec![InjectedFault::SqueezeFalsified],
            "expected exactly the squeeze-falsification blind spot, got {:?}",
            undetected
        );
    }

    // ── Gate 3B ─────────────────────────────────────────────────────────

    #[test]
    fn baseline_9of10_is_preserved() {
        // The structural (non-predictive) witness must STILL miss exactly the
        // squeeze-falsification class — Gate 3 result unchanged.
        let matrix = detection_matrix_with(40, 20, false);
        let undetected: Vec<_> = matrix
            .iter()
            .filter(|r| r.fault != InjectedFault::None && !r.detected)
            .map(|r| r.fault)
            .collect();
        assert_eq!(undetected, vec![InjectedFault::SqueezeFalsified]);
    }

    #[test]
    fn predictive_witness_reaches_10of10() {
        // With the clock-derived causal prediction, every injected fault class,
        // including SqueezeFalsified, is detected.
        let matrix = detection_matrix_with(40, 20, true);
        for r in &matrix {
            assert!(
                r.detected,
                "predictive witness missed {:?} ({:?})",
                r.fault, r.verdict
            );
        }
        // and the squeeze case is caught specifically by the prediction check
        let sq = matrix
            .iter()
            .find(|r| r.fault == InjectedFault::SqueezeFalsified)
            .unwrap();
        assert_eq!(sq.verdict, WitnessVerdict::SqueezePredictionMismatch);
    }

    #[test]
    fn zero_false_positives_on_unseen_valid_traces() {
        // The predictor must NOT flag valid traces it was never built against:
        // different start ticks and different lengths, all obeying the causal
        // law. Any false positive here would mean the check is overfit.
        for start in [0u64, 1, 3, 7, 10, 23, 100, 999] {
            for len in [1usize, 2, 5, 13, 40, 97] {
                let scan = healthy_scan_from(start, len);
                assert_eq!(
                    witness_predictive(&scan),
                    WitnessVerdict::Ok,
                    "false positive at start={} len={}",
                    start,
                    len
                );
            }
        }
    }

    #[test]
    fn prediction_uses_only_prior_information() {
        // Guard against a circular test: predict_squeeze depends ONLY on the
        // tick, so predicting a falsified observation still yields the CLEAN
        // expected value (the predictor cannot see the tampered s).
        let base = healthy_scan(40);
        let tampered = inject(&base, InjectedFault::SqueezeFalsified, 20);
        let expected = predict_squeeze(tampered[20].sequence);
        let clean = base[20].s;
        assert!(
            (expected - clean).abs() < 1e-12,
            "prediction must equal the clean value"
        );
        assert!(
            (tampered[20].s - clean).abs() > 0.1,
            "tampered value must differ"
        );
    }

    #[test]
    #[ignore]
    fn dump_predictive_matrix() {
        println!("Gate 3B predictive detection matrix (scan_len=40, fault at 20):");
        let mut detected = 0;
        let mut injected = 0;
        for r in detection_matrix_with(40, 20, true) {
            if r.fault != InjectedFault::None {
                injected += 1;
                if r.detected {
                    detected += 1;
                }
            }
            println!(
                "  {:16?}  detected={:5}  verdict={:?}",
                r.fault, r.detected, r.verdict
            );
        }
        println!(
            "detected {}/{} injected fault classes (predictive)",
            detected, injected
        );
    }

    #[test]
    #[ignore]
    fn dump_detection_matrix() {
        println!("Gate 3 detection matrix (scan_len=40, fault at index 20):");
        let mut detected = 0;
        let mut injected = 0;
        for r in detection_matrix(40, 20) {
            if r.fault != InjectedFault::None {
                injected += 1;
                if r.detected {
                    detected += 1;
                }
            }
            println!(
                "  {:16?}  detected={:5}  verdict={:?}",
                r.fault, r.detected, r.verdict
            );
        }
        println!("detected {}/{} injected fault classes", detected, injected);
    }

    #[test]
    fn direction_bit_tracks_squeeze_then_release() {
        let trace = squeeze_cycle_trace(1.0, 8);
        // first quarter should be squeezing (s rising from 0), later releasing.
        assert_eq!(trace[1].direction, RadialDirection::Squeezing);
        let releasing = trace
            .iter()
            .any(|r| r.direction == RadialDirection::Releasing);
        assert!(releasing, "cycle must contain a releasing phase");
    }
}
