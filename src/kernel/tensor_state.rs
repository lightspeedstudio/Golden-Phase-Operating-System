//! Cartesian tensor-state encoding (`feature = "tensor"`, default OFF).
//!
//! `A_kernel = S × M × V` with `|A| = 256`. This is a **Cartesian product**,
//! not an algebraic tensor product. See
//! `docs/research/PARALLEL_TENSOR_RESEARCH.md`.
//!
//! - **(S)** bits 0–1: existing [`PhaseState`]. `T_S` is `compose` / `reflect`
//!   from [`crate::core::gphase_core`] — this module does not own a compose
//!   table.
//! - **(M)** bits 2–3: [`SchedulerPhase`] = sched2 dispatch-epoch modulo 4.
//!   `TensorState` stores a **snapshot**; it is not a second clock.
//!   `T_M`: `m' = (m + 1) mod 4` only on `Sched2::advance`.
//! - **(V)** bits 4–7: [`CapabilityClass`] = `Q(rights) = rights & 0x0F`.
//!   Matching V **never** grants. `CapabilityTable::authorize` remains the
//!   only allow path.
//!
//! Packed form: [`TensorState`] is a `u32` whose bits 8–31 must be zero.
//! Canonical MMIO/hash/statechain payload is the little-endian low byte
//! (`as_u8`). All 256 values of that byte are valid. Path-1 TNSR CUR/RESULT
//! at `0x40000000+0x100` consume that byte via `board::zynq7000::g1p` on CPU0
//! only (`feature = "tensor"`). This file is not a register map.
//!
//! Non-claims: no CPU1, no sealed FPGA tensor, no `Kernel` field, no
//! `PH_tensor-proof`. `1/256` is blind random-match probability, not
//! cryptographic strength.

use crate::core::gphase_core::{compose, reflect, PhaseState};

const S_MASK: u32 = 0b0000_0011;
const M_SHIFT: u32 = 2;
const M_MASK: u32 = 0b0000_1100;
const V_SHIFT: u32 = 4;
const V_MASK: u32 = 0b1111_0000;
const PAYLOAD_MASK: u32 = 0x0000_00FF;

/// Two-bit sched2 dispatch-epoch snapshot. Not `StateClock.sector_id`.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SchedulerPhase {
    M0 = 0,
    M1 = 1,
    M2 = 2,
    M3 = 3,
}

impl SchedulerPhase {
    pub const ALL: [SchedulerPhase; 4] = [
        SchedulerPhase::M0,
        SchedulerPhase::M1,
        SchedulerPhase::M2,
        SchedulerPhase::M3,
    ];

    pub const fn from_u8(n: u8) -> Option<Self> {
        match n {
            0 => Some(SchedulerPhase::M0),
            1 => Some(SchedulerPhase::M1),
            2 => Some(SchedulerPhase::M2),
            3 => Some(SchedulerPhase::M3),
            _ => None,
        }
    }

    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// `M = sched2_advance_count mod 4`.
    pub const fn from_advance_count(advance_count: u64) -> Self {
        match advance_count % 4 {
            0 => SchedulerPhase::M0,
            1 => SchedulerPhase::M1,
            2 => SchedulerPhase::M2,
            _ => SchedulerPhase::M3,
        }
    }

    pub const fn wrapping_add_one(self) -> Self {
        match self {
            SchedulerPhase::M0 => SchedulerPhase::M1,
            SchedulerPhase::M1 => SchedulerPhase::M2,
            SchedulerPhase::M2 => SchedulerPhase::M3,
            SchedulerPhase::M3 => SchedulerPhase::M0,
        }
    }
}

/// Four-bit compact capability class. Label only — never a grant.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CapabilityClass(u8);

impl CapabilityClass {
    pub const fn from_u8(n: u8) -> Option<Self> {
        if n < 16 {
            Some(Self(n))
        } else {
            None
        }
    }

    /// `Q(rights) = rights & 0x0F`.
    pub const fn from_rights(rights: u32) -> Self {
        Self((rights & 0x0F) as u8)
    }

    pub const fn as_u8(self) -> u8 {
        self.0
    }
}

/// Packed `S × M × V`. Bits 8–31 reserved (must be zero).
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TensorState(u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TensorStateError {
    /// `raw & !0xFF != 0`.
    ReservedBitsSet,
}

impl TensorState {
    pub const PHASE_MASK: u32 = S_MASK;
    pub const SCHED_MASK: u32 = M_MASK;
    pub const CAPS_MASK: u32 = V_MASK;
    pub const USED_MASK: u32 = PAYLOAD_MASK;

    pub const fn new(
        phase: PhaseState,
        scheduler: SchedulerPhase,
        capability_class: CapabilityClass,
    ) -> Self {
        Self(
            (phase.as_u8() as u32 & 0x03)
                | ((scheduler.as_u8() as u32 & 0x03) << 2)
                | ((capability_class.as_u8() as u32 & 0x0F) << 4),
        )
    }

    pub const fn from_raw(raw: u32) -> Result<Self, TensorStateError> {
        if raw & !PAYLOAD_MASK != 0 {
            return Err(TensorStateError::ReservedBitsSet);
        }
        Ok(Self(raw))
    }

    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Canonical 1-byte little-endian payload.
    pub const fn as_u8(self) -> u8 {
        self.0 as u8
    }

    pub const fn as_le_bytes(self) -> [u8; 1] {
        [self.as_u8()]
    }

    pub fn phase(self) -> PhaseState {
        PhaseState::from_u8((self.0 & S_MASK) as u8).expect("bits 0-1 are a PhaseState")
    }

    pub fn scheduler(self) -> SchedulerPhase {
        SchedulerPhase::from_u8(((self.0 & M_MASK) >> M_SHIFT) as u8)
            .expect("bits 2-3 are a SchedulerPhase")
    }

    pub fn capability_class(self) -> CapabilityClass {
        CapabilityClass::from_u8(((self.0 & V_MASK) >> V_SHIFT) as u8)
            .expect("bits 4-7 are 0..=15")
    }

    /// `π_S(s, m, v) = s`.
    pub fn project_root(self) -> PhaseState {
        self.phase()
    }

    /// Snapshot from live S, a sched2 advance count, and a rights bitset.
    pub const fn snapshot(s: PhaseState, sched2_advance_count: u64, rights: u32) -> Self {
        Self::new(
            s,
            SchedulerPhase::from_advance_count(sched2_advance_count),
            CapabilityClass::from_rights(rights),
        )
    }

    /// `T_S`: compose on S only. Calls [`compose`], does not copy the table.
    pub fn t_s_compose(self, op: PhaseState) -> Self {
        Self::new(compose(self.phase(), op), self.scheduler(), self.capability_class())
    }

    /// `T_S`: reflect on S only. Calls [`reflect`].
    pub fn t_s_reflect(self) -> Self {
        Self::new(reflect(self.phase()), self.scheduler(), self.capability_class())
    }

    /// `T_M`: `m' = (m + 1) mod 4`. Does not change S or V.
    pub fn t_m(self) -> Self {
        Self::new(
            self.phase(),
            self.scheduler().wrapping_add_one(),
            self.capability_class(),
        )
    }

    /// `T_V`: refresh V from a rights bitset. Does not change S or M. Does
    /// not authorize.
    pub fn t_v(self, rights: u32) -> Self {
        Self::new(
            self.phase(),
            self.scheduler(),
            CapabilityClass::from_rights(rights),
        )
    }
}

/// Advisory operation a worker may evaluate. The result is not installed
/// until [`commit`] succeeds on CPU0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TensorOp {
    Compose(PhaseState),
    Reflect,
    AdvanceM,
    RefreshV(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TensorCommitError {
    /// Candidate did not match CPU0's local [`evaluate`] of the same op.
    VerifyFailed,
}

/// Worker/evaluate side. Pure. Does not write `Kernel`.
pub fn evaluate(current: TensorState, op: TensorOp) -> TensorState {
    match op {
        TensorOp::Compose(p) => current.t_s_compose(p),
        TensorOp::Reflect => current.t_s_reflect(),
        TensorOp::AdvanceM => current.t_m(),
        TensorOp::RefreshV(rights) => current.t_v(rights),
    }
}

/// CPU0 commit: accept `candidate` only if it equals [`evaluate`](current, op).
/// There is no `Kernel` tensor field; the caller that owns `&mut Kernel`
/// would store the `Ok` value later. Workers cannot commit.
pub fn commit(
    current: TensorState,
    candidate: TensorState,
    op: TensorOp,
) -> Result<TensorState, TensorCommitError> {
    if candidate != evaluate(current, op) {
        return Err(TensorCommitError::VerifyFailed);
    }
    Ok(candidate)
}

/// Accept a PL byte only when it equals CPU0 [`evaluate`]. Mismatch is not a commit.
pub fn commit_lockstep(
    current: TensorState,
    hw_byte: u8,
    op: TensorOp,
) -> Result<TensorState, TensorCommitError> {
    let candidate =
        TensorState::from_raw(hw_byte as u32).map_err(|_| TensorCommitError::VerifyFailed)?;
    commit(current, candidate, op)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::gphase_core::{topology_status, TopologyKind};

    const COMPOSE_TABLE: [[u8; 4]; 4] = [[0, 1, 2, 3], [1, 3, 3, 2], [2, 3, 3, 1], [3, 2, 1, 0]];

    #[test]
    fn layer0_census_still_52_same_12_reflection() {
        let mut same = 0;
        let mut reflection = 0;
        for a in PhaseState::ALL {
            for b in PhaseState::ALL {
                for c in PhaseState::ALL {
                    match topology_status(a, b, c).kind {
                        TopologyKind::Same => same += 1,
                        TopologyKind::Reflection => reflection += 1,
                    }
                }
            }
        }
        assert_eq!(same, 52);
        assert_eq!(reflection, 12);
    }

    #[test]
    fn compose_table_unchanged() {
        for a in PhaseState::ALL {
            for b in PhaseState::ALL {
                assert_eq!(
                    compose(a, b).as_u8(),
                    COMPOSE_TABLE[a.as_u8() as usize][b.as_u8() as usize]
                );
            }
        }
    }

    #[test]
    fn all_256_round_trip() {
        let mut n = 0u32;
        for s in PhaseState::ALL {
            for m in SchedulerPhase::ALL {
                for v in 0u8..16 {
                    n += 1;
                    let cls = CapabilityClass::from_u8(v).unwrap();
                    let packed = TensorState::new(s, m, cls);
                    let unpacked = TensorState::from_raw(packed.raw()).unwrap();
                    assert_eq!(unpacked.phase(), s);
                    assert_eq!(unpacked.scheduler(), m);
                    assert_eq!(unpacked.capability_class(), cls);
                    assert_eq!(unpacked.project_root(), s);
                    assert_eq!(unpacked.as_le_bytes(), [packed.as_u8()]);
                }
            }
        }
        assert_eq!(n, 256);
    }

    #[test]
    fn from_raw_rejects_reserved_bits() {
        assert_eq!(
            TensorState::from_raw(0x100),
            Err(TensorStateError::ReservedBitsSet)
        );
        assert_eq!(
            TensorState::from_raw(0xFFFF_FF00),
            Err(TensorStateError::ReservedBitsSet)
        );
        assert!(TensorState::from_raw(0xFF).is_ok());
    }

    #[test]
    fn scheduler_phase_rejects_holes() {
        assert!(SchedulerPhase::from_u8(4).is_none());
        assert!(CapabilityClass::from_u8(16).is_none());
        assert!(CapabilityClass::from_u8(15).is_some());
    }

    #[test]
    fn bit_layout_s_m_v() {
        let t = TensorState::new(
            PhaseState::Phi3,
            SchedulerPhase::M2,
            CapabilityClass::from_u8(9).unwrap(),
        );
        assert_eq!(t.phase(), PhaseState::Phi3);
        assert_eq!(t.scheduler(), SchedulerPhase::M2);
        assert_eq!(t.capability_class().as_u8(), 9);
        assert_eq!(t.raw(), 0b1001_10_11);
    }

    #[test]
    fn root_projection_independent_of_m_and_v() {
        for s in PhaseState::ALL {
            for m in SchedulerPhase::ALL {
                for v in 0u8..16 {
                    let t = TensorState::new(s, m, CapabilityClass::from_u8(v).unwrap());
                    assert_eq!(t.project_root(), s);
                }
            }
        }
    }

    #[test]
    fn pi_s_commutes_with_t_s_compose() {
        for raw in 0u32..=255 {
            let x = TensorState::from_raw(raw).unwrap();
            for op in PhaseState::ALL {
                let y = x.t_s_compose(op);
                assert_eq!(y.project_root(), compose(x.phase(), op));
                assert_eq!(y.scheduler(), x.scheduler());
                assert_eq!(y.capability_class(), x.capability_class());
            }
        }
    }

    #[test]
    fn pi_s_commutes_with_t_s_reflect() {
        for raw in 0u32..=255 {
            let x = TensorState::from_raw(raw).unwrap();
            let y = x.t_s_reflect();
            assert_eq!(y.project_root(), reflect(x.phase()));
            assert_eq!(y.scheduler(), x.scheduler());
            assert_eq!(y.capability_class(), x.capability_class());
        }
    }

    #[test]
    fn t_m_and_t_v_do_not_mutate_projected_s() {
        for raw in 0u32..=255 {
            let x = TensorState::from_raw(raw).unwrap();
            let after_m = x.t_m();
            assert_eq!(after_m.project_root(), x.phase());
            assert_eq!(after_m.scheduler(), x.scheduler().wrapping_add_one());
            assert_eq!(after_m.capability_class(), x.capability_class());
            for rights in 0u32..=16 {
                let after_v = x.t_v(rights);
                assert_eq!(after_v.project_root(), x.phase());
                assert_eq!(after_v.scheduler(), x.scheduler());
                assert_eq!(after_v.capability_class(), CapabilityClass::from_rights(rights));
            }
        }
    }

    #[test]
    fn scheduler_phase_is_advance_count_mod_4_not_sector_id() {
        assert_eq!(
            SchedulerPhase::from_advance_count(0),
            SchedulerPhase::M0
        );
        assert_eq!(
            SchedulerPhase::from_advance_count(1),
            SchedulerPhase::M1
        );
        assert_eq!(
            SchedulerPhase::from_advance_count(4),
            SchedulerPhase::M0
        );
        assert_eq!(
            SchedulerPhase::from_advance_count(10),
            SchedulerPhase::M2
        );
    }

    #[test]
    fn q_is_low_four_rights_bits() {
        assert_eq!(CapabilityClass::from_rights(0).as_u8(), 0);
        assert_eq!(CapabilityClass::from_rights(0b1111).as_u8(), 15);
        assert_eq!(CapabilityClass::from_rights(0b1_0001).as_u8(), 1);
    }

    #[test]
    fn snapshot_binds_s_epoch_and_rights() {
        let t = TensorState::snapshot(PhaseState::Phi1, 6, 0b0101);
        assert_eq!(t.phase(), PhaseState::Phi1);
        assert_eq!(t.scheduler(), SchedulerPhase::M2);
        assert_eq!(t.capability_class().as_u8(), 0b0101);
    }

    #[test]
    fn random_match_probability_is_one_over_256_not_crypto() {
        // Blind guess of a packed state: 1/256. Not entropy, not auth.
        assert_eq!(4 * 4 * 16, 256);
    }

    #[test]
    fn commit_accepts_matching_evaluate_and_rejects_forged() {
        let current = TensorState::snapshot(PhaseState::Phi0, 0, 0);
        let op = TensorOp::Compose(PhaseState::Phi1);
        let candidate = evaluate(current, op);
        assert_eq!(commit(current, candidate, op), Ok(candidate));
        let forged = current.t_m();
        assert_eq!(
            commit(current, forged, op),
            Err(TensorCommitError::VerifyFailed)
        );
        assert_eq!(
            commit(current, candidate, TensorOp::Reflect),
            Err(TensorCommitError::VerifyFailed)
        );
        assert_eq!(
            commit_lockstep(current, candidate.as_u8(), op),
            Ok(candidate)
        );
        assert_eq!(
            commit_lockstep(current, forged.as_u8(), op),
            Err(TensorCommitError::VerifyFailed)
        );
    }
}
