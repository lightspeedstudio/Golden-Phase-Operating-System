//! Fibonacci benchmark engines.

use crate::core::gphase_core::PhaseState;
use crate::kernel::phase_benchmark::bigint::{self, BigUint};
use crate::kernel::phase_benchmark::types::{EngineCounters, PAIR_A_LUT, PAIR_B_LUT};
use crate::kernel::phase_math::tables::{pair_index, triple_index, PhaseMathTables};
use crate::protocol::checksum::state_sheet;

#[derive(Debug, Clone)]
pub struct EngineOutput {
    pub current: BigUint,
    pub next: BigUint,
    pub counters: EngineCounters,
}

pub fn fib_binary_linear(n: u64) -> EngineOutput {
    let mut counters = EngineCounters {
        recurrence_levels: n as u32,
        ..Default::default()
    };
    let mut prev = bigint::zero();
    let mut curr = bigint::one();
    for _ in 0..n {
        counters.branches += 1;
        let next = bigint::add(&prev, &curr);
        counters.additions += 1;
        counters.allocations += 1;
        prev = curr;
        curr = next;
    }
    EngineOutput {
        current: prev,
        next: curr,
        counters,
    }
}

pub fn fib_binary_fast_doubling(n: u64) -> EngineOutput {
    if n == 0 {
        return EngineOutput {
            current: bigint::zero(),
            next: bigint::one(),
            counters: EngineCounters::default(),
        };
    }
    let mut a = bigint::zero();
    let mut b = bigint::one();
    let bit_len = 64 - n.leading_zeros();
    let mut counters = EngineCounters {
        recurrence_levels: bit_len,
        ..Default::default()
    };
    for shift in (0..bit_len).rev() {
        let two_b = bigint::shl1(&b);
        counters.allocations += 1;
        let two_b_minus_a = bigint::sub(&two_b, &a);
        counters.subtractions += 1;
        counters.allocations += 1;
        let c = bigint::mul(&a, &two_b_minus_a);
        counters.multiplications += 1;
        counters.allocations += 1;
        let a_sq = bigint::mul(&a, &a);
        counters.multiplications += 1;
        counters.allocations += 1;
        let b_sq = bigint::mul(&b, &b);
        counters.multiplications += 1;
        counters.allocations += 1;
        let d = bigint::add(&a_sq, &b_sq);
        counters.additions += 1;
        counters.allocations += 1;
        let bit = (n >> shift) & 1;
        counters.branches += 1;
        if bit == 1 {
            a = d.clone();
            b = bigint::add(&c, &d);
            counters.additions += 1;
            counters.allocations += 1;
        } else {
            a = c;
            b = d;
        }
    }
    EngineOutput {
        current: a,
        next: b,
        counters,
    }
}

pub fn fib_kairos_phase_doubling(n: u64, tables: &PhaseMathTables) -> EngineOutput {
    if n == 0 {
        return EngineOutput {
            current: bigint::zero(),
            next: bigint::one(),
            counters: EngineCounters::default(),
        };
    }
    let mut a = bigint::zero();
    let mut b = bigint::one();
    let mut phase = PhaseState::Phi0;
    let bit_len = 64 - n.leading_zeros();
    let mut counters = EngineCounters {
        recurrence_levels: bit_len,
        ..Default::default()
    };

    for shift in (0..bit_len).rev() {
        phase = advance_phase(phase, PhaseState::Phi1, tables, &mut counters);
        phase = advance_phase(phase, PhaseState::Phi2, tables, &mut counters);

        let two_b = bigint::shl1(&b);
        counters.allocations += 1;
        let two_b_minus_a = bigint::sub(&two_b, &a);
        counters.subtractions += 1;
        counters.allocations += 1;
        let c = bigint::mul(&a, &two_b_minus_a);
        counters.multiplications += 1;
        counters.allocations += 1;
        let a_sq = bigint::mul(&a, &a);
        counters.multiplications += 1;
        counters.allocations += 1;
        let b_sq = bigint::mul(&b, &b);
        counters.multiplications += 1;
        counters.allocations += 1;
        let d = bigint::add(&a_sq, &b_sq);
        counters.additions += 1;
        counters.allocations += 1;
        let c_plus_d = bigint::add(&c, &d);
        counters.additions += 1;
        counters.allocations += 1;

        phase = advance_phase(phase, PhaseState::Phi3, tables, &mut counters);
        counters.trajectory_records += 1;
        counters.kairos_events += 1;

        let bit = ((n >> shift) & 1) as usize;
        counters.phase_lookups += 2;
        counters.dynamic_branches += 0;
        let vals = [&c, &d, &c_plus_d];
        let idx_a = PAIR_A_LUT[bit];
        let idx_b = PAIR_B_LUT[bit];
        a = vals[idx_a].clone();
        b = vals[idx_b].clone();
        counters.allocations += 2;

        record_topology_lookups(phase, tables, &mut counters);
        phase = PhaseState::Phi1;
        counters.state_transitions += 1;
    }

    EngineOutput {
        current: a,
        next: b,
        counters,
    }
}

fn advance_phase(
    from: PhaseState,
    to: PhaseState,
    tables: &PhaseMathTables,
    counters: &mut EngineCounters,
) -> PhaseState {
    let idx = pair_index(from, to);
    counters.transition_lookups += 1;
    counters.phase_lookups += 1;
    let _legal = tables.transition[idx] != 0;
    counters.compose_lookups += 1;
    let _composed = tables.compose[pair_index(from, to)];
    counters.reflection_lookups += 1;
    let _reflected = tables.reflection[from.as_u8() as usize];
    counters.state_transitions += 1;
    counters.kairos_events += 1;
    to
}

fn record_topology_lookups(
    phase: PhaseState,
    tables: &PhaseMathTables,
    counters: &mut EngineCounters,
) {
    let next = PhaseState::Phi3;
    let idx = triple_index(phase, next, next);
    counters.topology_lookups += 1;
    counters.phase_lookups += 1;
    let _topo = tables.topology[idx];
    counters.chi_lookups += 1;
    let _chi = state_sheet(phase);
}
