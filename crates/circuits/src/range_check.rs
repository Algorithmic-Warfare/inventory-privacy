//! Range check gadgets for preventing field arithmetic wrap-around.
//!
//! In ZK circuits, all arithmetic happens in a finite field. Without range checks,
//! subtracting more than we have (e.g., 5 - 10) wraps around to a huge positive number.
//! These gadgets ensure values stay within expected bounds.
//!
//! We use 32-bit range checks which support values up to ~4.29 billion - sufficient for
//! game inventories where quantities rarely exceed millions.
//!
//! ## Optimization
//!
//! The optimized implementation allocates only the bits needed (32) as witnesses,
//! reconstructs the value, and verifies equality. This uses ~33 constraints instead
//! of ~884 for the naive approach that decomposes all 254 field bits.

use ark_ff::{BigInteger, PrimeField};
use ark_r1cs_std::fields::fp::FpVar;
use ark_r1cs_std::prelude::*;
use ark_relations::r1cs::{ConstraintSystemRef, SynthesisError};

/// Number of bits for range checks (32-bit values)
/// Supports quantities up to 4,294,967,295 (~4.29 billion)
pub const RANGE_BITS: usize = 32;

/// Enforce that a field element fits in `num_bits` bits.
///
/// This uses an optimized approach that only allocates the bits we need:
/// 1. Allocate `num_bits` boolean witnesses for the bit decomposition
/// 2. Reconstruct the value from these bits
/// 3. Enforce the reconstructed value equals the input
///
/// If the input value >= 2^num_bits, the constraint cannot be satisfied because
/// the reconstruction will produce a different value.
///
/// Constraint cost: ~num_bits constraints (vs ~254 for naive approach)
pub fn enforce_range<F: PrimeField>(
    cs: ConstraintSystemRef<F>,
    value: &FpVar<F>,
    num_bits: usize,
) -> Result<(), SynthesisError> {
    // Allocate only the bits we need as witnesses
    let bits: Vec<Boolean<F>> = (0..num_bits)
        .map(|i| {
            Boolean::new_witness(cs.clone(), || {
                let v = value.value().unwrap_or_default();
                let bigint = v.into_bigint();
                Ok(bigint.get_bit(i))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    // Reconstruct value from bits: value = sum(bit[i] * 2^i)
    // We build this as a linear combination for efficiency
    let mut coeff = F::one();
    let two = F::from(2u64);

    let mut reconstructed = FpVar::zero();
    for bit in &bits {
        // Add bit * 2^i to reconstructed
        let term = FpVar::constant(coeff);
        reconstructed += bit.select(&term, &FpVar::zero())?;
        coeff *= two;
    }

    // Enforce equality: if value >= 2^num_bits, this fails
    // because the reconstructed value will differ
    value.enforce_equal(&reconstructed)?;

    Ok(())
}

/// Enforce that a value is non-negative and fits in 32 bits.
///
/// This prevents underflow attacks where (small - large) wraps to a huge number.
/// 32 bits supports values up to ~4.29 billion, sufficient for game inventories.
///
/// Constraint cost: ~33 constraints
pub fn enforce_u32_range<F: PrimeField>(
    cs: ConstraintSystemRef<F>,
    value: &FpVar<F>,
) -> Result<(), SynthesisError> {
    enforce_range(cs, value, RANGE_BITS)
}

/// Alias for backward compatibility - now uses 32-bit range
#[inline]
pub fn enforce_u64_range<F: PrimeField>(
    cs: ConstraintSystemRef<F>,
    value: &FpVar<F>,
) -> Result<(), SynthesisError> {
    enforce_u32_range(cs, value)
}

/// Enforce that a >= b (non-negative difference).
///
/// This is done by checking that (a - b) fits in 32 bits.
/// If b > a, then (a - b) would wrap around to a huge number that doesn't fit.
///
/// Constraint cost: ~33 constraints
pub fn enforce_geq<F: PrimeField>(
    cs: ConstraintSystemRef<F>,
    a: &FpVar<F>,
    b: &FpVar<F>,
) -> Result<(), SynthesisError> {
    let diff = a - b;
    enforce_u32_range(cs, &diff)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_bn254::Fr;
    use ark_relations::r1cs::ConstraintSystem;
    use std::ops::Neg;

    #[test]
    fn test_range_check_valid() {
        let cs = ConstraintSystem::<Fr>::new_ref();

        // A value that fits in 32 bits
        let value = FpVar::new_witness(cs.clone(), || Ok(Fr::from(1000u64))).unwrap();

        enforce_u32_range(cs.clone(), &value).unwrap();

        assert!(cs.is_satisfied().unwrap());
        println!("Range check (32-bit) constraints: {}", cs.num_constraints());
    }

    #[test]
    fn test_range_check_max_u32() {
        let cs = ConstraintSystem::<Fr>::new_ref();

        // Maximum u32 value
        let value = FpVar::new_witness(cs.clone(), || Ok(Fr::from(u32::MAX as u64))).unwrap();

        enforce_u32_range(cs.clone(), &value).unwrap();

        assert!(cs.is_satisfied().unwrap());
    }

    #[test]
    fn test_range_check_exceeds_u32() {
        let cs = ConstraintSystem::<Fr>::new_ref();

        // Value that exceeds 32 bits (2^32)
        let value = FpVar::new_witness(cs.clone(), || Ok(Fr::from(1u64 << 32))).unwrap();

        enforce_u32_range(cs.clone(), &value).unwrap();

        // Should fail because 2^32 doesn't fit in 32 bits
        assert!(!cs.is_satisfied().unwrap());
    }

    #[test]
    fn test_range_check_overflow() {
        let cs = ConstraintSystem::<Fr>::new_ref();

        // A value that wraps around (simulating underflow)
        // This is p - 5 where p is the field modulus
        let wrapped_value = Fr::from(5u64).neg(); // -5 in the field = p - 5
        let value = FpVar::new_witness(cs.clone(), || Ok(wrapped_value)).unwrap();

        enforce_u32_range(cs.clone(), &value).unwrap();

        // This should fail because p - 5 doesn't fit in 32 bits
        assert!(!cs.is_satisfied().unwrap());
    }

    #[test]
    fn test_geq_valid() {
        let cs = ConstraintSystem::<Fr>::new_ref();

        let a = FpVar::new_witness(cs.clone(), || Ok(Fr::from(100u64))).unwrap();
        let b = FpVar::new_witness(cs.clone(), || Ok(Fr::from(50u64))).unwrap();

        enforce_geq(cs.clone(), &a, &b).unwrap();

        assert!(cs.is_satisfied().unwrap());
    }

    #[test]
    fn test_geq_equal() {
        let cs = ConstraintSystem::<Fr>::new_ref();

        let a = FpVar::new_witness(cs.clone(), || Ok(Fr::from(100u64))).unwrap();
        let b = FpVar::new_witness(cs.clone(), || Ok(Fr::from(100u64))).unwrap();

        enforce_geq(cs.clone(), &a, &b).unwrap();

        assert!(cs.is_satisfied().unwrap());
    }

    #[test]
    fn test_geq_invalid() {
        let cs = ConstraintSystem::<Fr>::new_ref();

        // a < b, so a - b wraps around
        let a = FpVar::new_witness(cs.clone(), || Ok(Fr::from(50u64))).unwrap();
        let b = FpVar::new_witness(cs.clone(), || Ok(Fr::from(100u64))).unwrap();

        enforce_geq(cs.clone(), &a, &b).unwrap();

        // This should fail because 50 - 100 wraps to a huge number
        assert!(!cs.is_satisfied().unwrap());
    }

    #[test]
    fn test_constraint_count() {
        let cs = ConstraintSystem::<Fr>::new_ref();

        let value = FpVar::new_witness(cs.clone(), || Ok(Fr::from(1000u64))).unwrap();
        enforce_u32_range(cs.clone(), &value).unwrap();

        let num_constraints = cs.num_constraints();
        println!("Optimized 32-bit range check: {} constraints", num_constraints);

        // Should be much less than 100 constraints
        assert!(
            num_constraints < 100,
            "Expected < 100 constraints, got {}",
            num_constraints
        );
    }
}
