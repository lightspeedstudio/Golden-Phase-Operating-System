//! Recursive golden shell ladder: q_(n+1) = q_n^2 / (1 + q_n^2).
//!
//! Mathematical identities only. Cross-level lattice coupling is experimental
//! and lives in `scripts/research/recursive_shell_lattice.py`.

use crate::core::gphase_core::{compose, phi, topology_status, PhaseState, TopologyKind, ALPHA};
use crate::platform::Vec;

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

/// Exact integer sequence a_n where q_n = 1/a_n when a_0 = 1.
/// Recurrence: a_(n+1) = a_n^2 + 1.
pub fn a_sequence(max_level: usize) -> Result<Vec<u128>, &'static str> {
    if max_level > 10 {
        return Err("max_level too large for u128 a_n");
    }
    let mut out = Vec::with_capacity(max_level + 1);
    let mut a: u128 = 1;
    out.push(a);
    for _ in 0..max_level {
        a = a
            .checked_mul(a)
            .and_then(|sq| sq.checked_add(1))
            .ok_or("a_n overflow")?;
        out.push(a);
    }
    Ok(out)
}

/// q_n as f64 (= 1/a_n for the integer ladder starting at a_0=1).
pub fn q_sequence(max_level: usize) -> Result<Vec<f64>, &'static str> {
    Ok(a_sequence(max_level)?
        .into_iter()
        .map(|a| 1.0 / (a as f64))
        .collect())
}

#[derive(Debug, Clone, Copy)]
pub struct ShellLevel {
    pub n: usize,
    pub a: u128,
    pub q: f64,
    pub alpha: f64,
    pub y_outer: f64,
    pub y_inner: f64,
}

impl ShellLevel {
    pub fn from_a(n: usize, a: u128) -> Self {
        let q = 1.0 / (a as f64);
        let p = phi();
        let denom = 1.0 + q * q;
        let y_outer = fsqrt(p / denom);
        let y_inner = q * y_outer;
        let alpha = (p * p * q * q) / (denom * denom);
        Self {
            n,
            a,
            q,
            alpha,
            y_outer,
            y_inner,
        }
    }

    pub fn residual_sum_sq(&self) -> f64 {
        fabs(self.y_outer * self.y_outer + self.y_inner * self.y_inner - phi())
    }

    pub fn residual_product(&self) -> f64 {
        fabs(self.y_outer * self.y_inner - fsqrt(self.alpha))
    }
}

pub fn shell_ladder(max_level: usize) -> Result<Vec<ShellLevel>, &'static str> {
    let a = a_sequence(max_level)?;
    Ok(a.into_iter()
        .enumerate()
        .map(|(n, a_n)| ShellLevel::from_a(n, a_n))
        .collect())
}

/// Associator census for the canonical coherent table (must stay 52/12/0).
pub fn associator_census() -> (u32, u32, u32) {
    let mut same = 0u32;
    let mut reflection = 0u32;
    let mut other = 0u32;
    for a in PhaseState::ALL {
        for b in PhaseState::ALL {
            for c in PhaseState::ALL {
                let t = topology_status(a, b, c);
                match t.kind {
                    TopologyKind::Same => same += 1,
                    TopologyKind::Reflection => reflection += 1,
                }
                // "other" would be uncontrolled disagreement; topology_status
                // only returns Same/Reflection for the coherent table.
                let _ = compose(a, b);
            }
        }
    }
    // Exhaustive 64 triples: coherent table is exactly 52 same / 12 reflection.
    let _ = other;
    (same, reflection, other)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::gphase_core::compose;

    #[test]
    fn a_recurrence() {
        let a = a_sequence(5).unwrap();
        assert_eq!(a, vec![1, 2, 5, 26, 677, 458_330]);
        for i in 0..5 {
            assert_eq!(a[i + 1], a[i] * a[i] + 1);
        }
    }

    #[test]
    fn q_starts_correctly() {
        let q = q_sequence(4).unwrap();
        assert!((q[0] - 1.0).abs() < 1e-12);
        assert!((q[1] - 0.5).abs() < 1e-12);
        assert!((q[2] - 0.2).abs() < 1e-12);
        assert!((q[3] - 1.0 / 26.0).abs() < 1e-12);
        assert!((q[4] - 1.0 / 677.0).abs() < 1e-12);
    }

    #[test]
    fn invariants_and_limits() {
        let ladder = shell_ladder(6).unwrap();
        for s in &ladder {
            assert!(s.residual_sum_sq() < 1e-9, "level {}", s.n);
            assert!(s.residual_product() < 1e-7, "level {}", s.n);
        }
        let last = ladder.last().unwrap();
        assert!(last.q < 1e-6);
        assert!(last.alpha < 1e-10);
        assert!(last.y_inner < 1e-6);
        assert!((last.y_outer - phi().sqrt()).abs() < 1e-6);
    }

    #[test]
    fn level3_does_not_change_compose_table() {
        // Shell levels do not redefine discrete compose; table is fixed.
        let expected = [[0u8, 1, 2, 3], [1, 3, 3, 2], [2, 3, 3, 1], [3, 2, 1, 0]];
        let _ladder = shell_ladder(3).unwrap();
        for a in PhaseState::ALL {
            for b in PhaseState::ALL {
                assert_eq!(
                    compose(a, b).as_u8(),
                    expected[a.as_u8() as usize][b.as_u8() as usize]
                );
            }
        }
    }

    #[test]
    fn associator_census_52_12_0() {
        let (s, r, o) = associator_census();
        assert_eq!((s, r, o), (52, 12, 0));
    }

    #[test]
    fn fine_structure_alpha_unchanged() {
        assert!((ALPHA - 7.2973525693e-3).abs() < 1e-15);
    }
}
