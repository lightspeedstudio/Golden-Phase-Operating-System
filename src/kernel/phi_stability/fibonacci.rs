//! Fast-doubling Fibonacci for PHI-STABILITY.

use crate::kernel::phi_stability::error::PhiStabilityError;
use crate::kernel::phi_stability::fixed::U1024;
use crate::kernel::phi_stability::types::FibonacciPair;

pub fn fib_pair_u1024(n: u64) -> Result<FibonacciPair<U1024>, PhiStabilityError> {
    if n == 0 {
        return Ok(FibonacciPair {
            current: U1024::ZERO,
            next: U1024::ONE,
        });
    }
    let mut a = U1024::ZERO;
    let mut b = U1024::ONE;
    let bit_len = 64 - n.leading_zeros();
    for shift in (0..bit_len).rev() {
        let two_b = b.shl1()?;
        let two_b_minus_a = two_b.sub(&a)?;
        let c = a.mul(&two_b_minus_a)?;
        let a_sq = a.mul(&a)?;
        let b_sq = b.mul(&b)?;
        let d = a_sq.add(&b_sq)?;
        if (n >> shift) & 1 == 1 {
            a = d;
            b = c.add(&d)?;
        } else {
            a = c;
            b = d;
        }
    }
    Ok(FibonacciPair {
        current: a,
        next: b,
    })
}

pub fn fib_u1024(n: u64) -> Result<U1024, PhiStabilityError> {
    Ok(fib_pair_u1024(n)?.current)
}

pub fn recurrence_steps(n: u64) -> u32 {
    if n == 0 {
        0
    } else {
        64 - n.leading_zeros()
    }
}

pub fn fib_linear_reference_u64(n: u64) -> Result<u64, PhiStabilityError> {
    if n > 93 {
        return Err(PhiStabilityError::TargetTooLarge);
    }
    if n == 0 {
        return Ok(0);
    }
    let mut a = 0u64;
    let mut b = 1u64;
    for _ in 1..n {
        let next = a.checked_add(b).ok_or(PhiStabilityError::IntegerOverflow)?;
        a = b;
        b = next;
    }
    Ok(b)
}

pub fn fib_pair_with_steps(n: u64) -> Result<(FibonacciPair<U1024>, u32), PhiStabilityError> {
    let pair = fib_pair_u1024(n)?;
    Ok((pair, recurrence_steps(n)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fib_small_values() {
        assert_eq!(fib_u1024(0).unwrap().to_decimal_string(), "0");
        assert_eq!(fib_u1024(1).unwrap().to_decimal_string(), "1");
        assert_eq!(fib_u1024(2).unwrap().to_decimal_string(), "1");
        assert_eq!(fib_u1024(10).unwrap().to_decimal_string(), "55");
        assert_eq!(fib_u1024(50).unwrap().to_decimal_string(), "12586269025");
    }

    #[test]
    fn fast_doubling_matches_linear() {
        for n in 0..=93 {
            let fast = fib_u1024(n).unwrap().to_decimal_string();
            let linear = fib_linear_reference_u64(n).unwrap().to_string();
            assert_eq!(fast, linear, "mismatch at n={n}");
        }
    }

    #[test]
    fn overflow_at_limit() {
        let max = crate::kernel::phi_stability::fixed::max_exact_target();
        assert!(fib_u1024(max).is_ok());
        assert!(fib_u1024(max + 1).is_err());
    }
}
