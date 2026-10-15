//! Canonical scalar backend adapter.

use crate::core::gphase_core::{compose, topology_status, TopologyKind};
use crate::kernel::phase_math::batch::{PhaseBatchInput, PhaseBatchOutput};
use crate::kernel::phase_math::tables::{pair_index, triple_index, PhaseMathTables};

pub fn process_batch(tables: &PhaseMathTables, batch: &PhaseBatchInput) -> PhaseBatchOutput {
    let mut out = PhaseBatchOutput::default();
    out.phase_results.reserve(batch.phase_ops.len());
    out.topology_results.reserve(batch.topology_ops.len());

    for op in &batch.phase_ops {
        let idx = pair_index(op.left, op.right);
        out.phase_results.push(
            crate::core::gphase_core::PhaseState::from_u8(tables.compose[idx])
                .expect("table phase"),
        );
    }

    for op in &batch.topology_ops {
        let idx = triple_index(op.a, op.b, op.c);
        out.topology_results.push(tables.topology[idx]);
    }

    out
}

pub fn verify_tables(tables: &PhaseMathTables) -> bool {
    for a in crate::core::gphase_core::PhaseState::ALL {
        for b in crate::core::gphase_core::PhaseState::ALL {
            if compose(a, b).as_u8() != tables.compose[pair_index(a, b)] {
                return false;
            }
        }
    }
    for a in crate::core::gphase_core::PhaseState::ALL {
        for b in crate::core::gphase_core::PhaseState::ALL {
            for c in crate::core::gphase_core::PhaseState::ALL {
                let raw = match topology_status(a, b, c).kind {
                    TopologyKind::Same => 0,
                    TopologyKind::Reflection => 1,
                };
                if raw != tables.topology[triple_index(a, b, c)] {
                    return false;
                }
            }
        }
    }
    true
}
