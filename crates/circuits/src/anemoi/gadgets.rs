//! Anemoi R1CS constraint gadgets for in-circuit verification.
//!
//! These gadgets enable proving Anemoi hash computations inside ZK circuits.
//! The key insight is that x^(1/5) can be efficiently constrained by:
//! 1. Witnessing the result w
//! 2. Constraining w^5 = x (only 2 multiplication constraints)

use ark_bn254::Fr;
use ark_r1cs_std::{
    prelude::*,
    fields::fp::FpVar,
};
use ark_relations::r1cs::{ConstraintSystemRef, SynthesisError};

use super::constants::{
    NUM_ROUNDS, GENERATOR, BETA, DELTA, ARK_C, ARK_D,
    exp_inv_alpha,
};

/// Anemoi state as circuit variables.
#[derive(Clone)]
pub struct AnemoiStateVar {
    pub x: FpVar<Fr>,
    pub y: FpVar<Fr>,
}

impl AnemoiStateVar {
    /// Create state from field element variables.
    pub fn from_vars(x: FpVar<Fr>, y: FpVar<Fr>) -> Self {
        Self { x, y }
    }
}

/// Compute x^5 in-circuit.
/// Costs 2 multiplication constraints.
fn exp_alpha_var(x: &FpVar<Fr>) -> Result<FpVar<Fr>, SynthesisError> {
    let x2 = x.square()?;    // 1 constraint
    let x4 = x2.square()?;   // 1 constraint
    let x5 = &x4 * x;        // 1 constraint
    Ok(x5)
}

/// Compute x^(1/5) in-circuit by witnessing and constraining.
///
/// We witness w = x^(1/5) and constrain w^5 = x.
/// This costs only 3 constraints (for w^5 computation) + 1 equality constraint.
fn exp_inv_alpha_var(
    cs: ConstraintSystemRef<Fr>,
    x: &FpVar<Fr>,
) -> Result<FpVar<Fr>, SynthesisError> {
    // Witness the inverse alpha result
    let w = FpVar::new_witness(cs.clone(), || {
        let x_val = x.value()?;
        Ok(exp_inv_alpha(x_val))
    })?;

    // Constrain w^5 = x
    let w5 = exp_alpha_var(&w)?;
    w5.enforce_equal(x)?;

    Ok(w)
}

/// Multiply by generator (g=3) in-circuit.
/// Optimized as x.double() + x = 3x
fn mul_by_generator_var(x: &FpVar<Fr>) -> Result<FpVar<Fr>, SynthesisError> {
    Ok(x.double()? + x)
}

/// Apply the Flystel S-box in-circuit.
///
/// The Flystel S-box operates on (x, y) and produces (x', y'):
/// 1. x = x - beta * y^2
/// 2. y = y - x^(1/alpha)
/// 3. x = x + beta * y^2 + delta
fn apply_sbox_var(
    cs: ConstraintSystemRef<Fr>,
    state: &mut AnemoiStateVar,
) -> Result<(), SynthesisError> {
    let beta = FpVar::constant(Fr::from(BETA));
    let delta = FpVar::constant(DELTA);

    // Step 1: x = x - beta * y^2
    let y_squared = state.y.square()?;
    state.x = &state.x - &beta * &y_squared;

    // Step 2: y = y - x^(1/alpha)
    let x_inv_alpha = exp_inv_alpha_var(cs, &state.x)?;
    state.y = &state.y - &x_inv_alpha;

    // Step 3: x = x + beta * y^2 + delta
    let new_y_squared = state.y.square()?;
    state.x = &state.x + &beta * &new_y_squared + &delta;

    Ok(())
}

/// Apply the MDS layer in-circuit.
///
/// For 2-cell state with g=3:
/// new_x = 3x + y
/// new_y = x + 4y
fn apply_mds_var(state: &mut AnemoiStateVar) -> Result<(), SynthesisError> {
    let g_plus_one = FpVar::constant(Fr::from(GENERATOR + 1));

    let new_x = mul_by_generator_var(&state.x)? + &state.y;
    let new_y = &state.x + &g_plus_one * &state.y;

    state.x = new_x;
    state.y = new_y;

    Ok(())
}

/// Apply round constants in-circuit.
fn apply_round_constants_var(
    state: &mut AnemoiStateVar,
    round: usize,
) -> Result<(), SynthesisError> {
    state.x = &state.x + FpVar::constant(ARK_C[round]);
    state.y = &state.y + FpVar::constant(ARK_D[round]);
    Ok(())
}

/// Execute the full Anemoi permutation in-circuit.
fn permutation_var(
    cs: ConstraintSystemRef<Fr>,
    state: &mut AnemoiStateVar,
) -> Result<(), SynthesisError> {
    for round in 0..NUM_ROUNDS {
        // ARK layer (add round constants)
        apply_round_constants_var(state, round)?;

        // MDS layer (linear diffusion)
        apply_mds_var(state)?;

        // S-box layer (Flystel)
        apply_sbox_var(cs.clone(), state)?;
    }

    // Final MDS layer
    apply_mds_var(state)?;

    Ok(())
}

/// Hash two field elements using Anemoi in-circuit.
///
/// This uses the Jive compression mode:
/// - Initialize state to (a, b)
/// - Apply permutation
/// - Return x + y
pub fn anemoi_hash_two_var(
    cs: ConstraintSystemRef<Fr>,
    a: &FpVar<Fr>,
    b: &FpVar<Fr>,
) -> Result<FpVar<Fr>, SynthesisError> {
    let mut state = AnemoiStateVar::from_vars(a.clone(), b.clone());
    permutation_var(cs, &mut state)?;

    // Jive output: x + y
    Ok(&state.x + &state.y)
}

/// Hash a single field element in-circuit.
pub fn anemoi_hash_var(
    cs: ConstraintSystemRef<Fr>,
    input: &FpVar<Fr>,
) -> Result<FpVar<Fr>, SynthesisError> {
    let zero = FpVar::zero();
    anemoi_hash_two_var(cs, input, &zero)
}

#[cfg(test)]
mod gadget_tests {
    use super::*;
    use super::super::native::anemoi_hash_two;
    use ark_ff::Field;
    use ark_relations::r1cs::ConstraintSystem;

    #[test]
    fn test_circuit_matches_native() {
        let a = Fr::from(123u64);
        let b = Fr::from(456u64);

        // Native computation
        let native_hash = anemoi_hash_two(a, b);

        // Circuit computation
        let cs = ConstraintSystem::<Fr>::new_ref();

        let a_var = FpVar::new_witness(cs.clone(), || Ok(a)).unwrap();
        let b_var = FpVar::new_witness(cs.clone(), || Ok(b)).unwrap();

        let hash_var = anemoi_hash_two_var(cs.clone(), &a_var, &b_var).unwrap();

        // Verify circuit is satisfied
        assert!(cs.is_satisfied().unwrap(), "Circuit should be satisfied");

        // Verify hash matches
        assert_eq!(
            hash_var.value().unwrap(),
            native_hash,
            "Circuit hash should match native hash"
        );

        println!("Anemoi 2:1 hash constraints: {}", cs.num_constraints());
    }

    #[test]
    fn test_constraint_count() {
        let cs = ConstraintSystem::<Fr>::new_ref();

        let a_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(1u64))).unwrap();
        let b_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(2u64))).unwrap();

        let _ = anemoi_hash_two_var(cs.clone(), &a_var, &b_var).unwrap();

        let num_constraints = cs.num_constraints();
        println!("Anemoi constraint count: {}", num_constraints);

        // Anemoi should have fewer constraints than Poseidon
        // With 21 rounds and optimized S-box: ~126 constraints
        assert!(
            num_constraints < 200,
            "Expected < 200 constraints, got {}",
            num_constraints
        );
    }

    #[test]
    fn test_exp_inv_alpha_correct() {
        let cs = ConstraintSystem::<Fr>::new_ref();

        let x = Fr::from(42u64);
        let x_var = FpVar::new_witness(cs.clone(), || Ok(x)).unwrap();

        let inv = exp_inv_alpha_var(cs.clone(), &x_var).unwrap();

        // Verify constraint is satisfied
        assert!(cs.is_satisfied().unwrap());

        // Verify inv^5 = x
        let inv_val = inv.value().unwrap();
        let inv5 = inv_val.pow([5u64]);
        assert_eq!(inv5, x, "inv^5 should equal x");
    }

    #[test]
    fn test_different_inputs_different_outputs() {
        let cs = ConstraintSystem::<Fr>::new_ref();

        let a1 = FpVar::new_witness(cs.clone(), || Ok(Fr::from(1u64))).unwrap();
        let b1 = FpVar::new_witness(cs.clone(), || Ok(Fr::from(2u64))).unwrap();
        let a2 = FpVar::new_witness(cs.clone(), || Ok(Fr::from(1u64))).unwrap();
        let b2 = FpVar::new_witness(cs.clone(), || Ok(Fr::from(3u64))).unwrap();

        let h1 = anemoi_hash_two_var(cs.clone(), &a1, &b1).unwrap();
        let h2 = anemoi_hash_two_var(cs.clone(), &a2, &b2).unwrap();

        assert!(cs.is_satisfied().unwrap());
        assert_ne!(
            h1.value().unwrap(),
            h2.value().unwrap(),
            "Different inputs should produce different hashes"
        );
    }
}
