//! EquationRoot, subsystem Merkle roots, and KernelStateRoot.

use crate::core::gphase_core::{
    compose, phi, reflect, topology_status, PhaseState, TopologyKind, ALPHA,
};
use crate::kernel::hash::{hash_pair, tagged_hash, StateHash};
use crate::platform::Vec;

pub const EQUATION_STRING: &str = "y^4 - phi*y^2 + alpha = 0";
pub const EQUATION_TAG: &str = "KAIROS_EQUATION_ROOT_V1";
pub const MERKLE_TAG: &str = "KAIROS_MERKLE_NODE_V1";
pub const KERNEL_ROOT_TAG: &str = "KAIROS_KERNEL_ROOT_V1";
pub const PRIME_LOG_TAG: &str = "KAIROS_PRIME_LOG_V1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EquationRoot {
    pub hash: StateHash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimeLogCommitment {
    pub hash: StateHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubsystemKind {
    Process,
    Memory,
    Filesystem,
    Vm,
    Syscall,
    Driver,
    /// Craft Mainframe subsystem (appended; preserves 0..=5 discriminants).
    Craft,
}

impl SubsystemKind {
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Process => 0,
            Self::Memory => 1,
            Self::Filesystem => 2,
            Self::Vm => 3,
            Self::Syscall => 4,
            Self::Driver => 5,
            Self::Craft => 6,
        }
    }

    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Process),
            1 => Some(Self::Memory),
            2 => Some(Self::Filesystem),
            3 => Some(Self::Vm),
            4 => Some(Self::Syscall),
            5 => Some(Self::Driver),
            6 => Some(Self::Craft),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelStateRoot {
    pub equation_root: StateHash,
    pub process_root: StateHash,
    pub memory_root: StateHash,
    pub filesystem_root: StateHash,
    pub vm_root: StateHash,
    pub syscall_root: StateHash,
    pub driver_root: StateHash,
    /// Added for Craft Mainframe; changes KERNEL_ROOT_TAG payload layout.
    pub craft_root: StateHash,
    pub hash: StateHash,
}

pub fn compose_table_bytes() -> [u8; 16] {
    let mut table = [0u8; 16];
    for a in PhaseState::ALL {
        for b in PhaseState::ALL {
            let idx = (a.as_u8() as usize) << 2 | b.as_u8() as usize;
            table[idx] = compose(a, b).as_u8();
        }
    }
    table
}

pub fn reflection_rule_bytes() -> [u8; 4] {
    let mut out = [0u8; 4];
    for x in PhaseState::ALL {
        out[x.as_u8() as usize] = reflect(x).as_u8();
    }
    out
}

pub fn topology_table_bytes() -> [u8; 64] {
    let mut out = [0u8; 64];
    let mut i = 0usize;
    for a in PhaseState::ALL {
        for b in PhaseState::ALL {
            for c in PhaseState::ALL {
                let status = topology_status(a, b, c);
                out[i] = match status.kind {
                    TopologyKind::Same => 0,
                    TopologyKind::Reflection => 1,
                };
                i += 1;
            }
        }
    }
    out
}

// no_std float shims (std uses the intrinsic; bare-metal uses libm) —
// same pattern as `core::gphase_core::fsqrt`.
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

fn equation_roots() -> [f64; 4] {
    let p = phi();
    let discriminant = p * p - 4.0 * ALPHA;
    let sqrt_disc = fsqrt(discriminant);
    let y2_outer = (p + sqrt_disc) / 2.0;
    let y2_inner = (p - sqrt_disc) / 2.0;
    [
        -fsqrt(y2_outer),
        -fsqrt(y2_inner),
        fsqrt(y2_inner),
        fsqrt(y2_outer),
    ]
}

fn log_roots() -> (f64, f64, f64) {
    let roots = equation_roots();
    let u_outer = roots[3] * roots[3];
    let u_inner = roots[2] * roots[2];
    let l_plus = fln(u_outer);
    let l_minus = fln(u_inner);
    let ln_phi = fln(phi());
    (l_plus, l_minus, ln_phi)
}

fn f64_bytes(v: f64) -> [u8; 8] {
    v.to_le_bytes()
}

pub fn compute_equation_root() -> EquationRoot {
    let roots = equation_roots();
    let (l_plus, l_minus, ln_phi) = log_roots();
    let ln_alpha = fln(ALPHA);
    let hash = tagged_hash(
        EQUATION_TAG,
        &[
            EQUATION_STRING.as_bytes(),
            &f64_bytes(phi()),
            &f64_bytes(ALPHA),
            &f64_bytes(ln_phi),
            &f64_bytes(ln_alpha),
            &f64_bytes(roots[0]),
            &f64_bytes(roots[1]),
            &f64_bytes(roots[2]),
            &f64_bytes(roots[3]),
            &f64_bytes(l_plus),
            &f64_bytes(l_minus),
            &compose_table_bytes(),
            &reflection_rule_bytes(),
            &topology_table_bytes(),
        ],
    );
    EquationRoot { hash }
}

pub fn compute_prime_log_commitment(equation: &EquationRoot) -> PrimeLogCommitment {
    let (l_plus, l_minus, ln_phi) = log_roots();
    let ln_alpha = fln(ALPHA);
    let prime_basis: [u8; 4] = [2, 3, 5, 7];
    let exponent_vector: [u8; 4] = [0, 0, 0, 0];
    let hash = tagged_hash(
        PRIME_LOG_TAG,
        &[
            &f64_bytes(ln_phi),
            &f64_bytes(ln_alpha),
            &f64_bytes(l_plus),
            &f64_bytes(l_minus),
            &prime_basis,
            &exponent_vector,
            &equation.hash,
        ],
    );
    PrimeLogCommitment { hash }
}

pub fn merkle_root(leaves: &[StateHash]) -> StateHash {
    if leaves.is_empty() {
        return tagged_hash(MERKLE_TAG, &[&[0u8; 32]]);
    }
    let mut level: Vec<StateHash> = leaves.to_vec();
    while level.len() > 1 {
        let mut next = Vec::new();
        let mut i = 0;
        while i < level.len() {
            let left = level[i];
            let right = if i + 1 < level.len() {
                level[i + 1]
            } else {
                level[i]
            };
            next.push(hash_pair(MERKLE_TAG, &left, &right));
            i += 2;
        }
        level = next;
    }
    level[0]
}

pub fn compute_kernel_state_root(
    equation_root: &StateHash,
    process_heads: &[StateHash],
    memory_heads: &[StateHash],
    filesystem_heads: &[StateHash],
    vm_heads: &[StateHash],
    syscall_heads: &[StateHash],
    driver_heads: &[StateHash],
    craft_heads: &[StateHash],
) -> KernelStateRoot {
    let process_root = merkle_root(process_heads);
    let memory_root = merkle_root(memory_heads);
    let filesystem_root = merkle_root(filesystem_heads);
    let vm_root = merkle_root(vm_heads);
    let syscall_root = merkle_root(syscall_heads);
    let driver_root = merkle_root(driver_heads);
    let craft_root = merkle_root(craft_heads);
    let hash = tagged_hash(
        KERNEL_ROOT_TAG,
        &[
            equation_root,
            &process_root,
            &memory_root,
            &filesystem_root,
            &vm_root,
            &syscall_root,
            &driver_root,
            &craft_root,
        ],
    );
    KernelStateRoot {
        equation_root: *equation_root,
        process_root,
        memory_root,
        filesystem_root,
        vm_root,
        syscall_root,
        driver_root,
        craft_root,
        hash,
    }
}
