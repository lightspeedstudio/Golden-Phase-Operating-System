//! Arbitrary-precision Fibonacci backend (hosted).

use crate::kernel::hash::tagged_hash;
use crate::kernel::hash::StateHash;
pub use num_bigint::BigUint;

pub fn zero() -> BigUint {
    BigUint::from(0u32)
}

pub fn one() -> BigUint {
    BigUint::from(1u32)
}

pub fn hash_value(value: &BigUint) -> StateHash {
    let bytes = value.to_bytes_be();
    tagged_hash("KAIROS_PHASE_BENCHMARK_FIB_V1", &[&bytes])
}

pub fn digit_count(value: &BigUint) -> u64 {
    if value == &zero() {
        1
    } else {
        value.to_string().len() as u64
    }
}

pub fn add(a: &BigUint, b: &BigUint) -> BigUint {
    a + b
}

pub fn sub(a: &BigUint, b: &BigUint) -> BigUint {
    a - b
}

pub fn mul(a: &BigUint, b: &BigUint) -> BigUint {
    a * b
}

pub fn shl1(a: &BigUint) -> BigUint {
    a << 1u32
}
