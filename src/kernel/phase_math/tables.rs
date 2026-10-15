//! Verified runtime tables derived from the canonical core.

use crate::core::gphase_core::{compose, reflect, topology_status, PhaseState, TopologyKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseMathTables {
    pub compose: [u8; 16],
    pub transition: [u8; 16],
    pub reflection: [u8; 4],
    pub topology: [u8; 64],
}

impl Default for PhaseMathTables {
    fn default() -> Self {
        Self::build()
    }
}

impl PhaseMathTables {
    pub fn build() -> Self {
        let mut compose_table = [0u8; 16];
        let mut transition_table = [0u8; 16];
        let mut reflection_table = [0u8; 4];
        let mut topology_table = [0u8; 64];

        for from in PhaseState::ALL {
            reflection_table[from.as_u8() as usize] = reflect(from).as_u8();
            for to in PhaseState::ALL {
                let idx = pair_index(from, to);
                compose_table[idx] = compose(from, to).as_u8();
                transition_table[idx] = legal_transition(from, to) as u8;
            }
        }

        let mut i = 0usize;
        for a in PhaseState::ALL {
            for b in PhaseState::ALL {
                for c in PhaseState::ALL {
                    topology_table[i] = match topology_status(a, b, c).kind {
                        TopologyKind::Same => 0,
                        TopologyKind::Reflection => 1,
                    };
                    i += 1;
                }
            }
        }

        Self {
            compose: compose_table,
            transition: transition_table,
            reflection: reflection_table,
            topology: topology_table,
        }
    }
}

pub fn pair_index(a: PhaseState, b: PhaseState) -> usize {
    ((a.as_u8() as usize) << 2) | b.as_u8() as usize
}

pub fn triple_index(a: PhaseState, b: PhaseState, c: PhaseState) -> usize {
    ((a.as_u8() as usize) << 4) | ((b.as_u8() as usize) << 2) | c.as_u8() as usize
}

pub fn legal_transition(from: PhaseState, to: PhaseState) -> bool {
    matches!(
        (from, to),
        (PhaseState::Phi0, PhaseState::Phi1)
            | (PhaseState::Phi1, PhaseState::Phi0)
            | (PhaseState::Phi1, PhaseState::Phi2)
            | (PhaseState::Phi2, PhaseState::Phi1)
            | (PhaseState::Phi2, PhaseState::Phi3)
            | (PhaseState::Phi3, PhaseState::Phi0)
            | (PhaseState::Phi3, PhaseState::Phi1)
    ) || from == to
}
