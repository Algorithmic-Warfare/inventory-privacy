//! Benchmarks comparing current vs optimized circuit implementations.
//!
//! Run with: cargo test -p circuits optimization_bench --release -- --nocapture

use ark_bn254::Fr;
use ark_ff::{BigInteger, PrimeField};
use ark_r1cs_std::fields::fp::FpVar;
use ark_r1cs_std::prelude::*;
use ark_relations::r1cs::{ConstraintSystem, ConstraintSystemRef, SynthesisError};

use crate::poseidon::poseidon_hash_two_var;

// ============================================================================
// RANGE CHECK COMPARISON
// ============================================================================

/// Current implementation: decomposes ALL 254 bits
fn range_check_current<F: PrimeField>(
    _cs: ConstraintSystemRef<F>,
    value: &FpVar<F>,
    num_bits: usize,
) -> Result<(), SynthesisError> {
    let value_bits = value.to_bits_le()?;
    for (i, bit) in value_bits.iter().enumerate() {
        if i >= num_bits {
            bit.enforce_equal(&Boolean::FALSE)?;
        }
    }
    Ok(())
}

/// Optimized: only allocate bits we need, reconstruct and verify
fn range_check_optimized<F: PrimeField>(
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

    // Reconstruct value from bits using linear combination
    // value = sum(bit[i] * 2^i)
    let mut coeff = F::one();
    let two = F::from(2u64);
    let mut lc_terms = Vec::with_capacity(num_bits);

    for bit in &bits {
        lc_terms.push((bit.clone(), coeff));
        coeff *= two;
    }

    // Build reconstructed value as linear combination
    let reconstructed = {
        let mut result = FpVar::zero();
        for (bit, coeff) in lc_terms {
            let term = FpVar::constant(coeff);
            result += bit.select(&term, &FpVar::zero())?;
        }
        result
    };

    // Enforce equality: if value >= 2^num_bits, this fails
    value.enforce_equal(&reconstructed)?;

    Ok(())
}

#[test]
fn bench_range_check_comparison() {
    println!("\n========================================");
    println!("RANGE CHECK OPTIMIZATION BENCHMARK");
    println!("========================================\n");

    // Test with 32-bit range check
    let test_value = Fr::from(1_000_000u64);

    // Current implementation
    let cs_current = ConstraintSystem::<Fr>::new_ref();
    let value_current = FpVar::new_witness(cs_current.clone(), || Ok(test_value)).unwrap();
    range_check_current(cs_current.clone(), &value_current, 32).unwrap();
    let current_constraints = cs_current.num_constraints();
    assert!(cs_current.is_satisfied().unwrap(), "Current implementation should be satisfied");

    // Optimized implementation
    let cs_optimized = ConstraintSystem::<Fr>::new_ref();
    let value_optimized = FpVar::new_witness(cs_optimized.clone(), || Ok(test_value)).unwrap();
    range_check_optimized(cs_optimized.clone(), &value_optimized, 32).unwrap();
    let optimized_constraints = cs_optimized.num_constraints();
    assert!(cs_optimized.is_satisfied().unwrap(), "Optimized implementation should be satisfied");

    println!("32-bit Range Check:");
    println!("  Current:   {} constraints", current_constraints);
    println!("  Optimized: {} constraints", optimized_constraints);
    println!("  Savings:   {} constraints ({:.1}%)",
        current_constraints.saturating_sub(optimized_constraints),
        100.0 * (current_constraints.saturating_sub(optimized_constraints)) as f64 / current_constraints as f64
    );

    // Verify optimized rejects overflow
    let cs_overflow = ConstraintSystem::<Fr>::new_ref();
    let overflow_value = Fr::from(1u64 << 32); // 2^32, doesn't fit in 32 bits
    let value_overflow = FpVar::new_witness(cs_overflow.clone(), || Ok(overflow_value)).unwrap();
    range_check_optimized(cs_overflow.clone(), &value_overflow, 32).unwrap();
    assert!(!cs_overflow.is_satisfied().unwrap(), "Optimized should reject overflow");
    println!("  Overflow rejection: ✓");

    // Verify optimized rejects negative (wrapped) values
    let cs_negative = ConstraintSystem::<Fr>::new_ref();
    let negative_value = Fr::from(0u64) - Fr::from(1u64); // -1 in field = huge positive
    let value_negative = FpVar::new_witness(cs_negative.clone(), || Ok(negative_value)).unwrap();
    range_check_optimized(cs_negative.clone(), &value_negative, 32).unwrap();
    assert!(!cs_negative.is_satisfied().unwrap(), "Optimized should reject negative wrap");
    println!("  Negative wrap rejection: ✓");

    println!();
}

// ============================================================================
// SMT DEPTH COMPARISON
// ============================================================================

/// Compute root from path with configurable depth
fn compute_root_from_path_depth(
    cs: ConstraintSystemRef<Fr>,
    leaf_hash: &FpVar<Fr>,
    siblings: &[FpVar<Fr>],
    indices: &[Boolean<Fr>],
) -> Result<FpVar<Fr>, SynthesisError> {
    let mut current = leaf_hash.clone();

    for (sibling, is_right) in siblings.iter().zip(indices.iter()) {
        let left = is_right.select(sibling, &current)?;
        let right = is_right.select(&current, sibling)?;
        current = poseidon_hash_two_var(cs.clone(), &left, &right)?;
    }

    Ok(current)
}

fn bench_smt_depth(depth: usize) -> usize {
    let cs = ConstraintSystem::<Fr>::new_ref();

    // Create dummy siblings and indices
    let siblings: Vec<FpVar<Fr>> = (0..depth)
        .map(|_| FpVar::new_witness(cs.clone(), || Ok(Fr::from(123u64))).unwrap())
        .collect();

    let indices: Vec<Boolean<Fr>> = (0..depth)
        .map(|i| Boolean::new_witness(cs.clone(), || Ok(i % 2 == 0)).unwrap())
        .collect();

    let leaf_hash = FpVar::new_witness(cs.clone(), || Ok(Fr::from(456u64))).unwrap();

    let before = cs.num_constraints();
    compute_root_from_path_depth(cs.clone(), &leaf_hash, &siblings, &indices).unwrap();
    let after = cs.num_constraints();

    after - before
}

#[test]
fn bench_smt_depth_comparison() {
    println!("\n========================================");
    println!("SMT DEPTH OPTIMIZATION BENCHMARK");
    println!("========================================\n");

    let depths = [8, 10, 12];
    let mut results = Vec::new();

    for &depth in &depths {
        let constraints = bench_smt_depth(depth);
        let items = 1usize << depth;
        results.push((depth, constraints, items));
    }

    println!("SMT Path Computation (compute_root_from_path):");
    println!("{:>6} | {:>12} | {:>12} | {:>12}", "Depth", "Items", "Constraints", "vs Depth-12");
    println!("{:-<6}-+-{:-<12}-+-{:-<12}-+-{:-<12}", "", "", "", "");

    let baseline = results.last().unwrap().1;
    for (depth, constraints, items) in &results {
        let savings = baseline.saturating_sub(*constraints);
        println!("{:>6} | {:>12} | {:>12} | {:>12}", depth, items, constraints,
            if *depth == 12 { "-".to_string() } else { format!("-{}", savings) });
    }

    // For verify_and_update (2x path computation + overhead)
    println!("\nEstimated verify_and_update savings:");
    let depth_12 = results[2].1;
    let depth_10 = results[1].1;
    let depth_8 = results[0].1;

    println!("  Depth 12 → 10: ~{} constraints", 2 * (depth_12 - depth_10));
    println!("  Depth 12 → 8:  ~{} constraints", 2 * (depth_12 - depth_8));
    println!();
}

// ============================================================================
// FULL CIRCUIT COMPARISON
// ============================================================================

#[test]
fn bench_full_circuit_breakdown() {
    println!("\n========================================");
    println!("FULL CIRCUIT CONSTRAINT BREAKDOWN");
    println!("========================================\n");

    // Measure individual components

    // 1. Single Poseidon hash
    let cs = ConstraintSystem::<Fr>::new_ref();
    let a = FpVar::new_witness(cs.clone(), || Ok(Fr::from(1u64))).unwrap();
    let b = FpVar::new_witness(cs.clone(), || Ok(Fr::from(2u64))).unwrap();
    let before = cs.num_constraints();
    let _ = poseidon_hash_two_var(cs.clone(), &a, &b).unwrap();
    let poseidon_constraints = cs.num_constraints() - before;

    // 2. Boolean select
    let cs = ConstraintSystem::<Fr>::new_ref();
    let cond = Boolean::new_witness(cs.clone(), || Ok(true)).unwrap();
    let x = FpVar::new_witness(cs.clone(), || Ok(Fr::from(1u64))).unwrap();
    let y = FpVar::new_witness(cs.clone(), || Ok(Fr::from(2u64))).unwrap();
    let before = cs.num_constraints();
    let _ = cond.select(&x, &y).unwrap();
    let select_constraints = cs.num_constraints() - before;

    // 3. is_eq
    let cs = ConstraintSystem::<Fr>::new_ref();
    let x = FpVar::new_witness(cs.clone(), || Ok(Fr::from(1u64))).unwrap();
    let y = FpVar::new_witness(cs.clone(), || Ok(Fr::from(1u64))).unwrap();
    let before = cs.num_constraints();
    let _ = x.is_eq(&y).unwrap();
    let is_eq_constraints = cs.num_constraints() - before;

    // 4. enforce_equal
    let cs = ConstraintSystem::<Fr>::new_ref();
    let x = FpVar::new_witness(cs.clone(), || Ok(Fr::from(1u64))).unwrap();
    let y = FpVar::new_witness(cs.clone(), || Ok(Fr::from(1u64))).unwrap();
    let before = cs.num_constraints();
    x.enforce_equal(&y).unwrap();
    let enforce_eq_constraints = cs.num_constraints() - before;

    // 5. Current range check (32-bit)
    let cs = ConstraintSystem::<Fr>::new_ref();
    let x = FpVar::new_witness(cs.clone(), || Ok(Fr::from(1000u64))).unwrap();
    let before = cs.num_constraints();
    range_check_current(cs.clone(), &x, 32).unwrap();
    let range_current = cs.num_constraints() - before;

    // 6. Optimized range check (32-bit)
    let cs = ConstraintSystem::<Fr>::new_ref();
    let x = FpVar::new_witness(cs.clone(), || Ok(Fr::from(1000u64))).unwrap();
    let before = cs.num_constraints();
    range_check_optimized(cs.clone(), &x, 32).unwrap();
    let range_optimized = cs.num_constraints() - before;

    println!("Component Constraint Costs:");
    println!("{:<30} {:>10}", "Component", "Constraints");
    println!("{:-<30}-{:-<10}", "", "");
    println!("{:<30} {:>10}", "Poseidon hash (2 inputs)", poseidon_constraints);
    println!("{:<30} {:>10}", "Boolean.select", select_constraints);
    println!("{:<30} {:>10}", "FpVar.is_eq", is_eq_constraints);
    println!("{:<30} {:>10}", "enforce_equal", enforce_eq_constraints);
    println!("{:<30} {:>10}", "Range check (current, 32-bit)", range_current);
    println!("{:<30} {:>10}", "Range check (optimized, 32-bit)", range_optimized);
    println!();

    // Compute per-level SMT cost
    let smt_per_level = 2 * select_constraints + poseidon_constraints;
    println!("SMT per-level cost: 2×select + hash = 2×{} + {} = {}",
        select_constraints, poseidon_constraints, smt_per_level);

    // Estimate StateTransition savings
    println!("\n--- StateTransition Circuit Savings Estimate ---");
    let current_state_transition = 8255; // From documentation
    let range_savings = 2 * (range_current - range_optimized); // 2 range checks
    let depth_12_to_10_savings = 2 * 2 * smt_per_level; // 2 levels × 2 path computations

    println!("Current total: ~{} constraints", current_state_transition);
    println!("Range check optimization: -{} constraints", range_savings);
    println!("Depth 12→10 (if applicable): -{} constraints", depth_12_to_10_savings);
    println!("Potential new total: ~{} constraints",
        current_state_transition - range_savings);
    println!();
}

// ============================================================================
// COMBINED IMPACT TEST
// ============================================================================

#[test]
fn bench_combined_optimizations() {
    println!("\n========================================");
    println!("COMBINED OPTIMIZATION IMPACT");
    println!("========================================\n");

    // Simulate a mini StateTransition-like circuit with current vs optimized

    // Current approach
    let cs_current = ConstraintSystem::<Fr>::new_ref();
    {
        let val1 = FpVar::new_witness(cs_current.clone(), || Ok(Fr::from(1000u64))).unwrap();
        let val2 = FpVar::new_witness(cs_current.clone(), || Ok(Fr::from(500u64))).unwrap();

        // Two range checks (current)
        range_check_current(cs_current.clone(), &val1, 32).unwrap();
        range_check_current(cs_current.clone(), &val2, 32).unwrap();

        // SMT path (depth 12)
        let siblings: Vec<FpVar<Fr>> = (0..12)
            .map(|_| FpVar::new_witness(cs_current.clone(), || Ok(Fr::from(123u64))).unwrap())
            .collect();
        let indices: Vec<Boolean<Fr>> = (0..12)
            .map(|i| Boolean::new_witness(cs_current.clone(), || Ok(i % 2 == 0)).unwrap())
            .collect();
        let leaf = FpVar::new_witness(cs_current.clone(), || Ok(Fr::from(456u64))).unwrap();
        let _ = compute_root_from_path_depth(cs_current.clone(), &leaf, &siblings, &indices).unwrap();
    }
    let current_total = cs_current.num_constraints();

    // Optimized approach
    let cs_optimized = ConstraintSystem::<Fr>::new_ref();
    {
        let val1 = FpVar::new_witness(cs_optimized.clone(), || Ok(Fr::from(1000u64))).unwrap();
        let val2 = FpVar::new_witness(cs_optimized.clone(), || Ok(Fr::from(500u64))).unwrap();

        // Two range checks (optimized)
        range_check_optimized(cs_optimized.clone(), &val1, 32).unwrap();
        range_check_optimized(cs_optimized.clone(), &val2, 32).unwrap();

        // SMT path (depth 12 - same for fair comparison)
        let siblings: Vec<FpVar<Fr>> = (0..12)
            .map(|_| FpVar::new_witness(cs_optimized.clone(), || Ok(Fr::from(123u64))).unwrap())
            .collect();
        let indices: Vec<Boolean<Fr>> = (0..12)
            .map(|i| Boolean::new_witness(cs_optimized.clone(), || Ok(i % 2 == 0)).unwrap())
            .collect();
        let leaf = FpVar::new_witness(cs_optimized.clone(), || Ok(Fr::from(456u64))).unwrap();
        let _ = compute_root_from_path_depth(cs_optimized.clone(), &leaf, &siblings, &indices).unwrap();
    }
    let optimized_total = cs_optimized.num_constraints();

    println!("Mini-circuit comparison (2 range checks + 1 SMT path):");
    println!("  Current:   {} constraints", current_total);
    println!("  Optimized: {} constraints", optimized_total);
    println!("  Savings:   {} constraints ({:.1}%)",
        current_total - optimized_total,
        100.0 * (current_total - optimized_total) as f64 / current_total as f64
    );

    // Extrapolate to full StateTransition
    let range_savings = current_total - optimized_total;
    // StateTransition has 3 range checks (new_qty, new_volume, capacity)
    let extrapolated_savings = (range_savings as f64 * 1.5) as usize;

    println!("\nExtrapolated StateTransition savings:");
    println!("  Current: ~8,255 constraints");
    println!("  After range optimization: ~{} constraints", 8255 - extrapolated_savings);
    println!();
}
