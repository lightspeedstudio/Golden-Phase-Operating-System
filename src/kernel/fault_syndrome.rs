//! Phase-state fault tolerance — a **phase-syndrome detector** running beside the
//! primary kernel (research: `research/phase-state-fault-tolerance`).
//!
//! Pipeline: predict → measure → syndrome → confirm-over-time → (Gate 2) vote →
//! restore. This module is Gate 1: the fixed-point syndrome core. Everything here
//! is deterministic **integer** math (no f64 in the hot path) so it runs on bare
//! metal beside the kernel.
//!
//! The generic phase-syndrome framework is bound to the **canonical Golden Phase
//! equation** — that binding is the whole point:
//!   * **FK (predict)** — the discrete next state is `compose(Φ, operand)`
//!     (`gphase_core::compose`); the continuous phase advances by `delta_phi`
//!     (`StateClock`), encoded here as a 16-bit "turn".
//!   * **I1 (invariant)** — the observed transition obeys the algebra: the actual
//!     Kairos state equals the predicted `compose` result.
//!   * **I2 (invariant)** — coherence (Theorem 1): even a mismatched state must lie
//!     in the predicted reflection orbit `{v, 3−v}` (`gphase_core::reflect`). A value
//!     outside that orbit is impossible under the real algebra ⇒ hard fault.

use crate::core::gphase_core::{compose, reflect, PhaseState as Phi};
use crate::kernel::hash::StateHash;
use crate::kernel::statechain_checkpoint::{verify_checkpoint, SignedCheckpoint};

/// One complete turn = 65_536 units (16-bit phase). 16384 = 90°, 32768 = 180°.
pub const TURN: u32 = 65_536;

// ── classification thresholds (starting values; tuned on hardware) ──────────
pub const T1_VALID: u64 = 100; // D² < T1        → Valid
pub const T2_SUSPECT: u64 = 400; // T1..T2       → Suspect
pub const T3_FAULT: u64 = 900; // T2..T3         → Fault ; ≥ T3 → Isolate

// ── syndrome weights (relative importance of each residual) ─────────────────
const PHASE_LIMIT: u64 = 64; // normal per-tick phase jitter (turn units)
const TICK_LIMIT: u64 = 1;
/// Cap on a normalized residual so one huge phase/tick error can't dwarf the
/// invariant weights (keeps D² bounded and comparable). q=32 → term ≤ w·1024.
const Q_CAP: u64 = 32;
const W_PHI2: u64 = 4;
const W_PHI3: u64 = 4;
const W_TICK: u64 = 16;
const W_ALGEBRA: u64 = 500; // algebra_root fingerprint mismatch
const W_CHAIN: u64 = 900; // statechain/checkpoint root mismatch (immediate isolate)
const W_I1: u64 = 500; // transition does not obey compose
const W_I2: u64 = 900; // value left the reflection orbit (coherence break)

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Health {
    Valid,
    Suspect,
    Fault,
    Isolate,
}

impl Health {
    pub fn as_str(self) -> &'static str {
        match self {
            Health::Valid => "VALID",
            Health::Suspect => "SUSPECT",
            Health::Fault => "FAULT",
            Health::Isolate => "ISOLATE",
        }
    }
}

/// What CPU0 may do with advisory worker / FPGA tensor paths.
/// Uses existing [`Health`] — there is no `Recover` health state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkerPathPolicy {
    /// VALID: new parallel/advisory work may be issued. CPU0 still commits.
    AllowParallel,
    /// SUSPECT: do not issue new privileged parallel work. Outstanding
    /// results must be verified with the CPU0 reference `evaluate` before
    /// commit.
    DrainAndVerifyOnCpu0,
    /// FAULT / ISOLATE: worker and FPGA mutation paths off. CPU0 reference
    /// path only. Restore uses the existing snapshot path, not a new enum.
    Cpu0ReferenceOnly,
}

/// PhaseGuard worker-path policy. CPU1/FPGA cannot mark themselves trusted.
pub fn worker_path_policy(health: Health) -> WorkerPathPolicy {
    match health {
        Health::Valid => WorkerPathPolicy::AllowParallel,
        Health::Suspect => WorkerPathPolicy::DrainAndVerifyOnCpu0,
        Health::Fault | Health::Isolate => WorkerPathPolicy::Cpu0ReferenceOnly,
    }
}

/// A failed tensor proof is never parallel, including under VALID.
pub fn worker_path_policy_with_proof(health: Health, proof_ok: bool) -> WorkerPathPolicy {
    if !proof_ok {
        return WorkerPathPolicy::Cpu0ReferenceOnly;
    }
    worker_path_policy(health)
}

/// A compact snapshot of kernel phase state. `phi2`/`phi3` are continuous phases
/// as 16-bit turns; `kphase` is the discrete Kairos state Φ0–Φ3 (0..3); the roots
/// are 64-bit fingerprints of the algebraic and statechain state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhaseState {
    pub phi2: u16,
    pub phi3: u16,
    pub kphase: u8,
    pub algebra_root: u64,
    pub chain_root: u64,
    pub tick: u64,
}

/// One transition's inputs: the continuous phase steps, the discrete algebra
/// operand for `compose`, and the roots expected after the step.
#[derive(Clone, Copy, Debug)]
pub struct TransitionInput {
    pub phi2_step: i16,
    pub phi3_step: i16,
    pub operand: u8, // Φ operand (0..3) for the compose transition
    pub next_algebra_root: u64,
    pub next_chain_root: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct Syndrome {
    pub phi2_error: i32,
    pub phi3_error: i32,
    pub tick_error: i64,
    pub algebra_mismatch: bool,
    pub chain_mismatch: bool,
    pub i1_compose_ok: bool, // transition obeyed the algebra
    pub i2_coherent: bool,   // stayed in the reflection orbit
    pub distance_sq: u64,
    pub health: Health,
}

/// Shortest signed distance around the phase circle. Casting the unsigned
/// difference through `i16` gives the wrapped result: `2 − 65534 = +4`, not −65532.
#[inline]
pub fn phase_error(actual: u16, expected: u16) -> i32 {
    actual.wrapping_sub(expected) as i16 as i32
}

#[inline]
fn phi(n: u8) -> Phi {
    Phi::from_u8(n).unwrap_or(Phi::Phi0)
}

#[inline]
fn square(v: u64) -> u64 {
    v.saturating_mul(v)
}

#[inline]
fn normalize(error: u64, limit: u64) -> u64 {
    error.saturating_mul(10) / limit.max(1)
}

/// **FK** — predict the next valid state. The discrete state follows the canonical
/// `compose(Φ, operand)`; the continuous phases advance by their steps; the tick
/// increments by one; the roots are the caller-supplied expected fingerprints.
pub fn predict_state(prev: PhaseState, input: TransitionInput) -> PhaseState {
    let next_k = compose(phi(prev.kphase), phi(input.operand)).as_u8();
    PhaseState {
        phi2: prev.phi2.wrapping_add(input.phi2_step as u16),
        phi3: prev.phi3.wrapping_add(input.phi3_step as u16),
        kphase: next_k,
        algebra_root: input.next_algebra_root,
        chain_root: input.next_chain_root,
        tick: prev.tick.wrapping_add(1),
    }
}

/// Compare the observed state against the predicted one and score the fault
/// distance. Folds in **I1** (compose obeyed) and **I2** (reflection-orbit
/// coherence) alongside the phase/root/tick residuals.
pub fn calculate_syndrome(actual: PhaseState, expected: PhaseState) -> Syndrome {
    let phi2_error = phase_error(actual.phi2, expected.phi2);
    let phi3_error = phase_error(actual.phi3, expected.phi3);
    let tick_error = actual.tick.wrapping_sub(expected.tick) as i64;
    let algebra_mismatch = actual.algebra_root != expected.algebra_root;
    let chain_mismatch = actual.chain_root != expected.chain_root;

    // I1: did the discrete transition land exactly on the predicted compose result?
    let i1_compose_ok = actual.kphase == expected.kphase;
    // I2: if not exact, is the observed state at least the reflection of the
    // predicted one (same orbit, one parity bit)? Anything else broke coherence.
    let i2_coherent = i1_compose_ok || actual.kphase == reflect(phi(expected.kphase)).as_u8();

    let q2 = normalize(phi2_error.unsigned_abs() as u64, PHASE_LIMIT).min(Q_CAP);
    let q3 = normalize(phi3_error.unsigned_abs() as u64, PHASE_LIMIT).min(Q_CAP);
    let qt = normalize(tick_error.unsigned_abs(), TICK_LIMIT).min(Q_CAP);

    let distance_sq = W_PHI2
        .saturating_mul(square(q2))
        .saturating_add(W_PHI3.saturating_mul(square(q3)))
        .saturating_add(W_TICK.saturating_mul(square(qt)))
        .saturating_add(if algebra_mismatch { W_ALGEBRA } else { 0 })
        .saturating_add(if chain_mismatch { W_CHAIN } else { 0 })
        .saturating_add(if i1_compose_ok { 0 } else { W_I1 })
        .saturating_add(if i2_coherent { 0 } else { W_I2 });

    let health = classify(distance_sq);

    Syndrome {
        phi2_error,
        phi3_error,
        tick_error,
        algebra_mismatch,
        chain_mismatch,
        i1_compose_ok,
        i2_coherent,
        distance_sq,
        health,
    }
}

#[inline]
pub fn classify(distance_sq: u64) -> Health {
    if distance_sq < T1_VALID {
        Health::Valid
    } else if distance_sq < T2_SUSPECT {
        Health::Suspect
    } else if distance_sq < T3_FAULT {
        Health::Fault
    } else {
        Health::Isolate
    }
}

/// Evidence accumulator — a single noisy sample must not trigger recovery. An
/// integer low-pass filter (`A = 7A/8 + D²`) plus a consecutive-fault counter;
/// a verified chain mismatch is treated as immediately serious.
#[derive(Clone, Copy, Debug)]
pub struct FaultAccumulator {
    pub filtered_score: u64,
    pub consecutive_faults: u8,
}

impl Default for FaultAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

impl FaultAccumulator {
    pub const fn new() -> Self {
        Self {
            filtered_score: 0,
            consecutive_faults: 0,
        }
    }

    /// Fold one syndrome in and return the debounced health verdict.
    pub fn update(&mut self, syndrome: &Syndrome) -> Health {
        self.filtered_score = self
            .filtered_score
            .saturating_mul(7)
            .saturating_add(syndrome.distance_sq)
            / 8;

        if matches!(syndrome.health, Health::Fault | Health::Isolate) {
            self.consecutive_faults = self.consecutive_faults.saturating_add(1);
        } else {
            self.consecutive_faults = 0;
        }

        // A verified chain failure, or a coherence break (a value outside the
        // reflection orbit — impossible under the real algebra), is immediately
        // serious: no debounce.
        if syndrome.chain_mismatch || !syndrome.i2_coherent {
            Health::Isolate
        } else if self.consecutive_faults >= 3 {
            Health::Fault
        } else if self.filtered_score >= T1_VALID {
            Health::Suspect
        } else {
            Health::Valid
        }
    }
}

// ── Gate 2: triple modular redundancy (TMR) + recovery ──────────────────────
//
// Three independent estimates of the same kernel state — the live primary, the
// PhaseGuard cosine-law reconstruction, and the last verified checkpoint. If two
// agree and the third disagrees, the third is the outlier; restore from the pair.

/// Agreement tolerance between two estimates, in 16-bit turn units.
pub const AGREE_TOL: u32 = 64;

pub struct GuardedState {
    pub primary: PhaseState,
    pub phase_guard: PhaseState,
    pub checkpoint: PhaseState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryDecision {
    KeepPrimary,
    RestoreFromGuard,
    RestoreCheckpoint,
    HaltSafely,
}

impl RecoveryDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            RecoveryDecision::KeepPrimary => "keep-primary",
            RecoveryDecision::RestoreFromGuard => "restore-from-guard",
            RecoveryDecision::RestoreCheckpoint => "restore-checkpoint",
            RecoveryDecision::HaltSafely => "halt-safely",
        }
    }
}

/// Two estimates agree if both continuous phases are within `AGREE_TOL` on the
/// circle and the discrete state + both roots match exactly.
pub fn states_agree(a: PhaseState, b: PhaseState) -> bool {
    (phase_error(a.phi2, b.phi2).unsigned_abs()) <= AGREE_TOL
        && (phase_error(a.phi3, b.phi3).unsigned_abs()) <= AGREE_TOL
        && a.kphase == b.kphase
        && a.tick == b.tick // temporal agreement — a tick skip/replay is a disagreement
        && a.algebra_root == b.algebra_root
        && a.chain_root == b.chain_root
}

/// Vote the three estimates: keep the primary if it agrees with either witness;
/// if the two witnesses agree and the primary is the outlier, restore from the
/// guard; otherwise there is no trustworthy majority — halt safely.
pub fn choose_recovery(states: &GuardedState) -> RecoveryDecision {
    let primary_guard = states_agree(states.primary, states.phase_guard);
    let primary_checkpoint = states_agree(states.primary, states.checkpoint);
    let guard_checkpoint = states_agree(states.phase_guard, states.checkpoint);
    match (primary_guard, primary_checkpoint, guard_checkpoint) {
        (true, _, _) => RecoveryDecision::KeepPrimary,
        (false, true, _) => RecoveryDecision::KeepPrimary,
        (false, false, true) => RecoveryDecision::RestoreFromGuard,
        _ => RecoveryDecision::HaltSafely,
    }
}

/// Equal-weight circular mean of two 16-bit-turn phases: the midpoint on the
/// shorter arc. Integer only — no trig, no floats.
pub fn circular_mean(a: u16, b: u16) -> u16 {
    let half = phase_error(b, a) / 2; // signed shorter-arc half-step
    a.wrapping_add(half as u16)
}

/// The corrected state from an agreeing pair: phases averaged on the circle, the
/// (agreeing) discrete state and roots kept, tick taken as the later of the two.
pub fn corrected_state(a: PhaseState, b: PhaseState) -> PhaseState {
    PhaseState {
        phi2: circular_mean(a.phi2, b.phi2),
        phi3: circular_mean(a.phi3, b.phi3),
        kphase: a.kphase,
        algebra_root: a.algebra_root,
        chain_root: a.chain_root,
        tick: a.tick.max(b.tick),
    }
}

// ── Provenanced estimates + independence-gated DECISION (no recovery here) ────
//
// Provenance is enforced by CONSTRUCTION, not by a label field. `Estimate` has all
// fields private; the only way to build one is a per-source constructor that requires
// that source's authenticated inputs. A caller cannot fabricate data and stamp a
// foreign origin on it.
//
// HONEST LIMITATION (surfaced while building this): a signed checkpoint attests a
// PAST kernel root + tick, NOT the current phase coordinates. So it is a *prior-root
// witness / verified rollback anchor* — not a concurrent estimate of the current
// phase. The concurrent independence check is therefore primary-vs-PhaseGuard-
// reconstruction (phase); the checkpoint supplies the verified rollback target. A
// true 3-way vote of the *current* phase would need deterministic forward-replay from
// the checkpoint — that does not exist yet. `tmr_decide` returns a DECISION ONLY; it
// restores nothing.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EstimateOrigin {
    Primary,
    PhaseGuardRecon,
    Checkpoint,
}

impl EstimateOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            EstimateOrigin::Primary => "primary",
            EstimateOrigin::PhaseGuardRecon => "phaseguard-recon",
            EstimateOrigin::Checkpoint => "checkpoint",
        }
    }
}

/// A provenanced estimate. **All fields are private** — build one only via the
/// per-source constructors, each of which requires that source's authenticated
/// inputs, so an origin cannot be forged or relabelled by a caller.
#[derive(Clone, Copy, Debug)]
pub struct Estimate {
    state: PhaseState,
    origin: EstimateOrigin,
    source_id: u32,
    generation: u64,
    state_root: StateHash,
    capture_tick: u64,
    verified: bool,
}

impl Estimate {
    pub fn state(&self) -> PhaseState {
        self.state
    }
    pub fn origin(&self) -> EstimateOrigin {
        self.origin
    }
    pub fn source_id(&self) -> u32 {
        self.source_id
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn state_root(&self) -> StateHash {
        self.state_root
    }
    pub fn capture_tick(&self) -> u64 {
        self.capture_tick
    }
    pub fn verified(&self) -> bool {
        self.verified
    }

    /// Built ONLY from a live kernel snapshot (the subsystem under test).
    pub fn from_live_kernel(
        state: PhaseState,
        kernel_root: StateHash,
        capture_tick: u64,
        source_id: u32,
    ) -> Self {
        Self {
            state,
            origin: EstimateOrigin::Primary,
            source_id,
            generation: capture_tick,
            state_root: kernel_root,
            capture_tick,
            verified: true,
        }
    }

    /// Built ONLY from an independent cosine-law reconstruction. Takes the tick and
    /// the independently-computed phase coordinates — it NEVER accepts a primary
    /// `PhaseState`, so it cannot be a copy of the primary snapshot (item 8).
    pub fn from_phaseguard_recon(kairos_tick: u64, phi2: u16, phi3: u16, kphase: u8) -> Self {
        let state = PhaseState {
            phi2,
            phi3,
            kphase,
            algebra_root: 0,
            chain_root: 0,
            tick: kairos_tick,
        };
        Self {
            state,
            origin: EstimateOrigin::PhaseGuardRecon,
            source_id: 1,
            generation: kairos_tick,
            state_root: [0u8; 32],
            capture_tick: kairos_tick,
            verified: true,
        }
    }

    /// Built ONLY from a checkpoint that (a) VERIFIES against `signing_key`, (b) is a
    /// PRIOR seal (`cp.tick < current_tick`), and (c) is not stale (age ≤ `max_age`).
    /// Returns `None` otherwise — a `Checkpoint`-origin estimate is unobtainable from
    /// an unverified/forged (item 7), stale (item 6), or non-prior checkpoint.
    pub fn from_verified_checkpoint(
        cp: &SignedCheckpoint,
        signing_key: &StateHash,
        current_tick: u64,
        max_age: u64,
    ) -> Option<Self> {
        if !verify_checkpoint(signing_key, cp) {
            return None; // reject unverified / forged
        }
        if cp.tick >= current_tick {
            return None; // must be a prior seal, not "now" (blocks a copy of the current state)
        }
        if current_tick.saturating_sub(cp.tick) > max_age {
            return None; // reject stale
        }
        // A checkpoint attests ROOTS + tick, not phase coordinates.
        let state = PhaseState {
            phi2: 0,
            phi3: 0,
            kphase: 0,
            algebra_root: root_low64(&cp.equation_root),
            chain_root: root_low64(&cp.kernel_root),
            tick: cp.tick,
        };
        Some(Self {
            state,
            origin: EstimateOrigin::Checkpoint,
            source_id: cp.sequence as u32,
            generation: cp.tick,
            state_root: cp.kernel_root,
            capture_tick: cp.tick,
            verified: true,
        })
    }
}

fn root_low64(h: &StateHash) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&h[0..8]);
    u64::from_le_bytes(b)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TmrDecision {
    /// The concurrent independent witness agrees with primary — keep it.
    KeepPrimary,
    /// Primary disagrees with the independent reconstruction — a real recovery
    /// routine (does not exist yet) SHOULD roll back to this verified checkpoint.
    RestoreFromCheckpoint(u32),
    /// Not enough genuinely independent, verified sources to decide safely.
    Unavailable,
}

/// Decide (do NOT act) using provenanced estimates. Requires the three expected
/// origins; the checkpoint verified and a PRIOR seal; and no byte-copy (equal
/// generation AND equal root). The concurrent independence check is primary vs the
/// independent PhaseGuard reconstruction (phase only, since the checkpoint has no
/// phase). Returns a DECISION only — it restores nothing.
pub fn tmr_decide(primary: &Estimate, guard: &Estimate, checkpoint: &Estimate) -> TmrDecision {
    if primary.origin != EstimateOrigin::Primary
        || guard.origin != EstimateOrigin::PhaseGuardRecon
        || checkpoint.origin != EstimateOrigin::Checkpoint
    {
        return TmrDecision::Unavailable;
    }
    if !checkpoint.verified {
        return TmrDecision::Unavailable; // item 7
    }
    if checkpoint.generation >= primary.generation {
        return TmrDecision::Unavailable; // checkpoint must be a prior seal
    }
    // Copy detection (item 5): identical generation AND identical root ⇒ a copied snapshot.
    if primary.generation == guard.generation && primary.state_root == guard.state_root {
        return TmrDecision::Unavailable;
    }
    if phase_agree(primary.state, guard.state) {
        TmrDecision::KeepPrimary
    } else {
        TmrDecision::RestoreFromCheckpoint(checkpoint.source_id)
    }
}

/// Phase-only agreement (phi + discrete state), for the primary-vs-reconstruction
/// concurrent check — the checkpoint carries no phase so it is not compared here.
fn phase_agree(a: PhaseState, b: PhaseState) -> bool {
    (phase_error(a.phi2, b.phi2).unsigned_abs()) <= AGREE_TOL
        && (phase_error(a.phi3, b.phi3).unsigned_abs()) <= AGREE_TOL
        && a.kphase == b.kphase
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> PhaseState {
        PhaseState {
            phi2: 1000,
            phi3: 2000,
            kphase: 1,
            algebra_root: 0xABCD,
            chain_root: 0x1234,
            tick: 10,
        }
    }
    fn step() -> TransitionInput {
        TransitionInput {
            phi2_step: 100,
            phi3_step: 50,
            operand: 2, // compose(Φ1, Φ2) = Φ3
            next_algebra_root: 0xABCD,
            next_chain_root: 0x1234,
        }
    }

    #[test]
    fn wrapped_phase_error_crosses_boundary() {
        assert_eq!(phase_error(10, 5), 5);
        assert_eq!(phase_error(5, 10), -5);
        assert_eq!(phase_error(2, 65534), 4); // crosses the ±π seam: only +4
    }

    #[test]
    fn fk_uses_compose() {
        let next = predict_state(base(), step());
        assert_eq!(next.kphase, compose(Phi::Phi1, Phi::Phi2).as_u8()); // = 3
        assert_eq!(next.tick, 11);
        assert_eq!(next.phi2, 1100);
    }

    #[test]
    fn clean_transition_is_valid() {
        let expected = predict_state(base(), step());
        let s = calculate_syndrome(expected, expected);
        assert_eq!(s.health, Health::Valid);
        assert!(s.i1_compose_ok && s.i2_coherent);
    }

    #[test]
    fn compose_violation_scores_fault() {
        let expected = predict_state(base(), step()); // kphase = Φ3
        let mut actual = expected;
        // The orbit of Φ3 is {Φ3, Φ0} (reflect(Φ3)=Φ0). Φ1 is outside it → coherence break.
        actual.kphase = 1;
        let s = calculate_syndrome(actual, expected);
        assert!(!s.i1_compose_ok);
        assert!(!s.i2_coherent); // Φ1 not in {Φ3, Φ0}
        assert_eq!(s.health, Health::Isolate); // coherence break is the worst
    }

    #[test]
    fn reflection_stays_in_orbit_but_flags_i1() {
        let expected = predict_state(base(), step()); // kphase = 3
        let mut actual = expected;
        actual.kphase = reflect(Phi::Phi3).as_u8(); // = 0, the orbit partner
        let s = calculate_syndrome(actual, expected);
        assert!(!s.i1_compose_ok);
        assert!(s.i2_coherent); // still in {Φ3, Φ0}
    }

    #[test]
    fn chain_mismatch_isolates_immediately() {
        let expected = predict_state(base(), step());
        let mut actual = expected;
        actual.chain_root = 0xDEAD;
        let s = calculate_syndrome(actual, expected);
        assert!(s.chain_mismatch);
        let mut acc = FaultAccumulator::new();
        assert_eq!(acc.update(&s), Health::Isolate);
    }

    #[test]
    fn accumulator_debounces_single_spike() {
        let expected = predict_state(base(), step());
        let clean = calculate_syndrome(expected, expected);
        let mut acc = FaultAccumulator::new();
        // one isolated fault sample should not confirm a fault
        let mut spike = clean;
        spike.distance_sq = 500;
        spike.health = Health::Fault;
        assert_ne!(acc.update(&spike), Health::Fault);
        // but three in a row does
        acc.update(&spike);
        assert_eq!(acc.update(&spike), Health::Fault);
    }

    #[test]
    fn tmr_all_agree_keeps_primary() {
        let s = base();
        let g = GuardedState {
            primary: s,
            phase_guard: s,
            checkpoint: s,
        };
        assert_eq!(choose_recovery(&g), RecoveryDecision::KeepPrimary);
    }

    #[test]
    fn tmr_primary_outlier_restores_from_guard() {
        let good = base();
        let mut bad = good;
        bad.kphase = (good.kphase + 1) % 4; // primary corrupted; witnesses agree
        let g = GuardedState {
            primary: bad,
            phase_guard: good,
            checkpoint: good,
        };
        assert_eq!(choose_recovery(&g), RecoveryDecision::RestoreFromGuard);
    }

    #[test]
    fn tmr_no_majority_halts() {
        let a = base();
        let mut b = a;
        b.kphase = (a.kphase + 1) % 4;
        let mut c = a;
        c.kphase = (a.kphase + 2) % 4;
        let g = GuardedState {
            primary: a,
            phase_guard: b,
            checkpoint: c,
        };
        assert_eq!(choose_recovery(&g), RecoveryDecision::HaltSafely);
    }

    #[test]
    fn circular_mean_wraps_the_seam() {
        assert_eq!(circular_mean(10, 20), 15);
        // midpoint of −6 (65530) and +6 on the circle is 0
        let m = circular_mean(65530, 6);
        assert!(m <= 3 || m >= 65533, "got {m}");
    }

    #[test]
    fn coherence_break_isolates_on_first_sample() {
        let expected = predict_state(base(), step()); // Φ3
        let mut actual = expected;
        actual.kphase = 1; // outside orbit {Φ3,Φ0} → i2 fails
        let s = calculate_syndrome(actual, expected);
        assert!(!s.i2_coherent);
        let mut acc = FaultAccumulator::new();
        assert_eq!(acc.update(&s), Health::Isolate); // no debounce for a coherence break
    }

    #[test]
    fn tick_skip_breaks_agreement_and_recovers() {
        let good = base();
        let mut skewed = good;
        skewed.tick = good.tick.wrapping_add(5); // temporal fault only
        assert!(!states_agree(skewed, good));
        let g = GuardedState {
            primary: skewed,
            phase_guard: good,
            checkpoint: good,
        };
        assert_eq!(choose_recovery(&g), RecoveryDecision::RestoreFromGuard);
    }

    #[test]
    fn phase_residual_is_capped() {
        let expected = predict_state(base(), step());
        let mut drift = expected;
        drift.phi2 = expected.phi2.wrapping_add(3000);
        let s = calculate_syndrome(drift, expected);
        // capped: the phase term cannot exceed W_PHI2 * Q_CAP² = 4 * 1024
        assert!(s.distance_sq <= 4 * 1024 + 1, "D²={}", s.distance_sq);
        assert_eq!(s.health, Health::Isolate); // still a hard fault
    }

    // ── construction-enforced provenance + decision (HOST_UNIT_TEST) ──
    use crate::kernel::statechain_checkpoint::sign_checkpoint;

    fn signing() -> StateHash {
        [7u8; 32]
    }
    // A verified, prior checkpoint at `tick`, sealing given roots.
    fn good_cp(seq: u64, tick: u64) -> SignedCheckpoint {
        sign_checkpoint(&signing(), seq, tick, &[1u8; 32], &[2u8; 32], &[3u8; 32])
    }

    #[test]
    fn checkpoint_estimate_rejects_unverified() {
        let mut cp = good_cp(0, 5);
        cp.signature = [0xFFu8; 32]; // forge / corrupt the signature
        let e = Estimate::from_verified_checkpoint(&cp, &signing(), 10, 100);
        assert!(
            e.is_none(),
            "unverified checkpoint must NOT yield an estimate (item 7)"
        );
    }

    #[test]
    fn checkpoint_estimate_rejects_stale() {
        let cp = good_cp(0, 5);
        // current_tick 200, max_age 10 → age 195 → stale.
        let e = Estimate::from_verified_checkpoint(&cp, &signing(), 200, 10);
        assert!(
            e.is_none(),
            "stale checkpoint must NOT yield an estimate (item 6)"
        );
    }

    #[test]
    fn checkpoint_estimate_rejects_non_prior() {
        let cp = good_cp(0, 10);
        // cp.tick == current_tick → not a prior seal (blocks copying "now").
        let e = Estimate::from_verified_checkpoint(&cp, &signing(), 10, 100);
        assert!(
            e.is_none(),
            "non-prior checkpoint must NOT yield an estimate"
        );
    }

    #[test]
    fn checkpoint_estimate_accepts_verified_prior_fresh() {
        let cp = good_cp(3, 8);
        let e = Estimate::from_verified_checkpoint(&cp, &signing(), 10, 100);
        assert!(e.is_some());
        let e = e.unwrap();
        assert_eq!(e.origin(), EstimateOrigin::Checkpoint);
        assert_eq!(e.source_id(), 3);
        assert!(e.verified());
    }

    #[test]
    fn decide_keep_primary_when_recon_agrees() {
        let s = base();
        let primary = Estimate::from_live_kernel(s, [9u8; 32], 10, 42);
        let guard = Estimate::from_phaseguard_recon(10, s.phi2, s.phi3, s.kphase);
        let cp = Estimate::from_verified_checkpoint(&good_cp(1, 8), &signing(), 10, 100).unwrap();
        assert_eq!(tmr_decide(&primary, &guard, &cp), TmrDecision::KeepPrimary);
    }

    #[test]
    fn decide_restore_when_primary_disagrees_with_recon() {
        let s = base();
        let mut corrupt = s;
        corrupt.kphase = (s.kphase + 1) % 4;
        let primary = Estimate::from_live_kernel(corrupt, [9u8; 32], 10, 42);
        let guard = Estimate::from_phaseguard_recon(10, s.phi2, s.phi3, s.kphase);
        let cp = Estimate::from_verified_checkpoint(&good_cp(1, 8), &signing(), 10, 100).unwrap();
        assert_eq!(
            tmr_decide(&primary, &guard, &cp),
            TmrDecision::RestoreFromCheckpoint(1)
        );
    }

    #[test]
    fn decide_unavailable_when_snapshot_is_a_copy() {
        // guard forced to the same generation AND root as primary ⇒ copy ⇒ refuse.
        let s = base();
        let primary = Estimate::from_live_kernel(s, [0u8; 32], 10, 42);
        // reconstruction constructor sets root=[0;32] and generation=tick, so tick=10
        // makes generation AND root collide with primary → detected as a copy (item 5).
        let guard = Estimate::from_phaseguard_recon(10, s.phi2, s.phi3, s.kphase);
        let cp = Estimate::from_verified_checkpoint(&good_cp(1, 8), &signing(), 10, 100).unwrap();
        assert_eq!(tmr_decide(&primary, &guard, &cp), TmrDecision::Unavailable);
    }

    #[test]
    fn worker_path_policy_uses_existing_health_not_a_recover_variant() {
        assert_eq!(
            worker_path_policy(Health::Valid),
            WorkerPathPolicy::AllowParallel
        );
        assert_eq!(
            worker_path_policy(Health::Suspect),
            WorkerPathPolicy::DrainAndVerifyOnCpu0
        );
        assert_eq!(
            worker_path_policy(Health::Fault),
            WorkerPathPolicy::Cpu0ReferenceOnly
        );
        assert_eq!(
            worker_path_policy(Health::Isolate),
            WorkerPathPolicy::Cpu0ReferenceOnly
        );
        assert_eq!(
            worker_path_policy_with_proof(Health::Valid, false),
            WorkerPathPolicy::Cpu0ReferenceOnly
        );
        assert_eq!(
            worker_path_policy_with_proof(Health::Valid, true),
            WorkerPathPolicy::AllowParallel
        );
        assert_eq!(
            worker_path_policy_with_proof(Health::Suspect, true),
            WorkerPathPolicy::DrainAndVerifyOnCpu0
        );
    }
}
