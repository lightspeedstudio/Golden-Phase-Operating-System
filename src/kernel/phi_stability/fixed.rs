//! Fixed-width U1024 integer for baremetal PHI-STABILITY.

use crate::kernel::phi_stability::error::PhiStabilityError;
use crate::platform::{String, Vec};

pub const U1024_BITS: u16 = 1024;
pub const U1024_LIMBS: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct U1024 {
    limbs: [u64; U1024_LIMBS],
}

impl U1024 {
    pub const ZERO: Self = Self {
        limbs: [0; U1024_LIMBS],
    };
    pub const ONE: Self = Self {
        limbs: {
            let mut v = [0u64; U1024_LIMBS];
            v[0] = 1;
            v
        },
    };

    pub fn from_u64(value: u64) -> Self {
        let mut limbs = [0u64; U1024_LIMBS];
        limbs[0] = value;
        Self { limbs }
    }

    pub fn is_zero(&self) -> bool {
        self.limbs.iter().all(|&l| l == 0)
    }

    pub fn bit_length(&self) -> u32 {
        for (i, limb) in self.limbs.iter().enumerate().rev() {
            if *limb != 0 {
                return (i as u32) * 64 + (64 - limb.leading_zeros());
            }
        }
        0
    }

    pub fn add(&self, other: &Self) -> Result<Self, PhiStabilityError> {
        let mut out = [0u64; U1024_LIMBS];
        let mut carry = 0u128;
        for (i, slot) in out.iter_mut().enumerate().take(U1024_LIMBS) {
            let sum = self.limbs[i] as u128 + other.limbs[i] as u128 + carry;
            *slot = sum as u64;
            carry = sum >> 64;
        }
        if carry != 0 {
            return Err(PhiStabilityError::IntegerOverflow);
        }
        Ok(Self { limbs: out })
    }

    pub fn sub(&self, other: &Self) -> Result<Self, PhiStabilityError> {
        let mut out = [0u64; U1024_LIMBS];
        let mut borrow = 0i128;
        for (i, slot) in out.iter_mut().enumerate().take(U1024_LIMBS) {
            let diff = self.limbs[i] as i128 - other.limbs[i] as i128 - borrow;
            if diff < 0 {
                *slot = (diff + (1i128 << 64)) as u64;
                borrow = 1;
            } else {
                *slot = diff as u64;
                borrow = 0;
            }
        }
        if borrow != 0 {
            return Err(PhiStabilityError::IntegerOverflow);
        }
        Ok(Self { limbs: out })
    }

    pub fn mul(&self, other: &Self) -> Result<Self, PhiStabilityError> {
        let mut wide = [0u128; U1024_LIMBS * 2];
        for i in 0..U1024_LIMBS {
            if self.limbs[i] == 0 {
                continue;
            }
            let mut carry = 0u128;
            for j in 0..U1024_LIMBS {
                let idx = i + j;
                let prod = wide[idx] + self.limbs[i] as u128 * other.limbs[j] as u128 + carry;
                wide[idx] = prod & ((1u128 << 64) - 1);
                carry = prod >> 64;
            }
            if carry != 0 {
                let idx = i + U1024_LIMBS;
                if idx >= wide.len() {
                    return Err(PhiStabilityError::IntegerOverflow);
                }
                wide[idx] += carry;
                if wide[idx] >> 64 != 0 {
                    return Err(PhiStabilityError::IntegerOverflow);
                }
            }
        }
        for limb in wide.iter().skip(U1024_LIMBS) {
            if *limb != 0 {
                return Err(PhiStabilityError::IntegerOverflow);
            }
        }
        let mut limbs = [0u64; U1024_LIMBS];
        for (i, slot) in limbs.iter_mut().enumerate() {
            *slot = wide[i] as u64;
        }
        Ok(Self { limbs })
    }

    pub fn shl1(&self) -> Result<Self, PhiStabilityError> {
        let mut out = [0u64; U1024_LIMBS];
        let mut carry = 0u64;
        for (i, slot) in out.iter_mut().enumerate().take(U1024_LIMBS) {
            let new_carry = self.limbs[i] >> 63;
            *slot = (self.limbs[i] << 1) | carry;
            carry = new_carry;
        }
        if carry != 0 {
            return Err(PhiStabilityError::IntegerOverflow);
        }
        Ok(Self { limbs: out })
    }

    pub fn to_be_bytes(&self) -> [u8; 128] {
        let mut out = [0u8; 128];
        for (i, limb) in self.limbs.iter().enumerate().rev() {
            let bytes = limb.to_be_bytes();
            let base = (U1024_LIMBS - 1 - i) * 8;
            out[base..base + 8].copy_from_slice(&bytes);
        }
        out
    }

    pub fn digit_count(&self) -> u64 {
        if self.is_zero() {
            return 1;
        }
        let mut copy = *self;
        let mut digits = 0u64;
        while !copy.is_zero() {
            copy = div_rem_u64(&copy, 10).0;
            digits += 1;
        }
        digits
    }

    pub fn to_decimal_string(&self) -> String {
        if self.is_zero() {
            return "0".into();
        }
        let mut copy = *self;
        let mut digits = Vec::new();
        while !copy.is_zero() {
            let (quotient, rem) = div_rem_u64(&copy, 10);
            digits.push(b'0' + rem as u8);
            copy = quotient;
        }
        digits.reverse();
        String::from_utf8(digits).unwrap_or_else(|_| "?".into())
    }
}

fn div_rem_u64(value: &U1024, divisor: u64) -> (U1024, u64) {
    let mut remainder = 0u128;
    let mut out = [0u64; U1024_LIMBS];
    for i in (0..U1024_LIMBS).rev() {
        let wide = (remainder << 64) | value.limbs[i] as u128;
        let q = wide / divisor as u128;
        remainder = wide % divisor as u128;
        out[i] = q as u64;
    }
    (U1024 { limbs: out }, remainder as u64)
}

pub fn max_exact_target() -> u64 {
    let mut n = 1u64;
    loop {
        let candidate = n + 1;
        if crate::kernel::phi_stability::fibonacci::fib_pair_u1024(candidate).is_err() {
            return n;
        }
        n = candidate;
        if n > 10_000 {
            break;
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn u1024_add_mul() {
        let a = U1024::from_u64(55);
        let b = U1024::from_u64(89);
        let sum = a.add(&b).unwrap();
        assert_eq!(sum.to_decimal_string(), "144");
        let prod = U1024::from_u64(12).mul(&U1024::from_u64(13)).unwrap();
        assert_eq!(prod.to_decimal_string(), "156");
    }
}
