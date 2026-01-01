//! Validation tests for R1CS Optimizer correctness.
//!
//! These tests use circuits with KNOWN issues to verify the optimizer
//! correctly detects and reduces them.

use ark_bn254::Fr;
use ark_relations::r1cs::{
    ConstraintSynthesizer, ConstraintSystemRef, SynthesisError, ConstraintSystem,
    LinearCombination, Variable,
};
use r1cs_optimizer::{Optimizer, OptimizerConfig, DeduplicationPass, ReductionPass};

// ============================================================================
// TEST CIRCUITS WITH KNOWN ISSUES
// ============================================================================

/// Circuit with exactly 3 duplicate constraints.
struct KnownDuplicatesCircuit;

impl ConstraintSynthesizer<Fr> for KnownDuplicatesCircuit {
    fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
        let x = cs.new_witness_variable(|| Ok(Fr::from(5u64)))?;
        let y = cs.new_witness_variable(|| Ok(Fr::from(3u64)))?;
        // Make z a public input so it's not eliminated as dead
        let z = cs.new_input_variable(|| Ok(Fr::from(15u64)))?;

        // Original + 2 duplicates
        for _ in 0..3 {
            cs.enforce_constraint(
                LinearCombination::from(x),
                LinearCombination::from(y),
                LinearCombination::from(z),
            )?;
        }

        // Different constraint - w is also public to avoid elimination
        let w = cs.new_input_variable(|| Ok(Fr::from(25u64)))?;
        cs.enforce_constraint(
            LinearCombination::from(x),
            LinearCombination::from(x),
            LinearCombination::from(w),
        )?;

        Ok(())
    }
}

/// Circuit with exactly 2 constant constraints.
struct KnownConstantsCircuit;

impl ConstraintSynthesizer<Fr> for KnownConstantsCircuit {
    fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
        let one = Variable::One;

        // Constant: 5 * 3 = 15
        cs.enforce_constraint(
            LinearCombination::zero() + (Fr::from(5u64), one),
            LinearCombination::zero() + (Fr::from(3u64), one),
            LinearCombination::zero() + (Fr::from(15u64), one),
        )?;

        // Constant: 1 * 7 = 7
        cs.enforce_constraint(
            LinearCombination::from(one),
            LinearCombination::zero() + (Fr::from(7u64), one),
            LinearCombination::zero() + (Fr::from(7u64), one),
        )?;

        // Non-constant - make y a public input so it's not eliminated
        let x = cs.new_witness_variable(|| Ok(Fr::from(2u64)))?;
        let y = cs.new_input_variable(|| Ok(Fr::from(4u64)))?;
        cs.enforce_constraint(
            LinearCombination::from(x),
            LinearCombination::from(x),
            LinearCombination::from(y),
        )?;

        Ok(())
    }
}

/// Circuit with exactly 3 boolean constraints.
struct KnownBooleansCircuit;

impl ConstraintSynthesizer<Fr> for KnownBooleansCircuit {
    fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
        for val in [1u64, 0, 1] {
            let b = cs.new_witness_variable(|| Ok(Fr::from(val)))?;
            cs.enforce_constraint(
                LinearCombination::from(b),
                LinearCombination::from(b),
                LinearCombination::from(b),
            )?;
        }

        // Non-boolean
        let x = cs.new_witness_variable(|| Ok(Fr::from(3u64)))?;
        let y = cs.new_witness_variable(|| Ok(Fr::from(9u64)))?;
        cs.enforce_constraint(
            LinearCombination::from(x),
            LinearCombination::from(x),
            LinearCombination::from(y),
        )?;

        Ok(())
    }
}

// ============================================================================
// VALIDATION TESTS
// ============================================================================

#[test]
fn validate_duplicate_detection_and_reduction() {
    let cs = ConstraintSystem::<Fr>::new_ref();
    KnownDuplicatesCircuit.generate_constraints(cs.clone()).unwrap();
    cs.finalize();

    // Before: 4 constraints (3 duplicates + 1 unique)
    let optimizer = Optimizer::from_cs(cs);
    assert_eq!(optimizer.stats().num_constraints, 4);

    let result = optimizer.optimize();

    // After: 2 constraints (1 from duplicate group + 1 unique)
    assert_eq!(result.final_constraints, 2);
    assert_eq!(result.constraints_reduced(), 2);
}

#[test]
fn validate_constant_folding() {
    let cs = ConstraintSystem::<Fr>::new_ref();
    KnownConstantsCircuit.generate_constraints(cs.clone()).unwrap();
    cs.finalize();

    let optimizer = Optimizer::from_cs(cs);
    let stats = optimizer.stats();

    assert_eq!(stats.num_constraints, 3);
    assert_eq!(stats.constant_constraints, 2);

    let result = optimizer.optimize();

    // Both constant constraints should be removed
    assert_eq!(result.final_constraints, 1);
}

#[test]
fn validate_boolean_detection() {
    let cs = ConstraintSystem::<Fr>::new_ref();
    KnownBooleansCircuit.generate_constraints(cs.clone()).unwrap();
    cs.finalize();

    let optimizer = Optimizer::from_cs(cs);
    let stats = optimizer.stats();

    assert_eq!(stats.num_constraints, 4);
    assert_eq!(stats.boolean_constraints, 3);
}

#[test]
fn validate_no_false_positives() {
    // Clean circuit with no optimization opportunities
    let cs = ConstraintSystem::<Fr>::new_ref();

    let a = cs.new_witness_variable(|| Ok(Fr::from(2u64))).unwrap();
    let b = cs.new_witness_variable(|| Ok(Fr::from(3u64))).unwrap();
    let c = cs.new_witness_variable(|| Ok(Fr::from(6u64))).unwrap();

    cs.enforce_constraint(
        LinearCombination::from(a),
        LinearCombination::from(b),
        LinearCombination::from(c),
    ).unwrap();

    cs.finalize();

    let optimizer = Optimizer::from_cs(cs);
    let result = optimizer.with_config(OptimizerConfig::safe()).optimize();

    // No reductions should occur on clean circuit
    assert_eq!(result.original_constraints, result.final_constraints);
}

#[test]
fn validate_reduction_pass_directly() {
    use r1cs_optimizer::ConstraintMatrix;

    let cs = ConstraintSystem::<Fr>::new_ref();
    KnownDuplicatesCircuit.generate_constraints(cs.clone()).unwrap();
    cs.finalize();

    let matrix = ConstraintMatrix::from_cs(cs);
    let pass = DeduplicationPass::new();

    // Scan
    let matches = pass.scan(&matrix);
    assert!(!matches.is_empty());

    // Reduce
    let reduced = pass.reduce(matrix.clone(), &matches);
    assert!(reduced.num_constraints() < matrix.num_constraints());
}

#[test]
fn regression_inventory_circuits() {
    use inventory_circuits::{StateTransitionCircuit, ItemExistsSMTCircuit, CapacitySMTCircuit};

    // StateTransition
    {
        let cs = ConstraintSystem::<Fr>::new_ref();
        StateTransitionCircuit::empty().generate_constraints(cs.clone()).unwrap();
        cs.finalize();

        let stats = Optimizer::from_cs(cs).stats();
        assert!(
            stats.num_constraints > 7000 && stats.num_constraints < 8000,
            "StateTransition: expected ~7520, got {}",
            stats.num_constraints
        );
    }

    // ItemExistsSMT
    {
        let cs = ConstraintSystem::<Fr>::new_ref();
        ItemExistsSMTCircuit::empty().generate_constraints(cs.clone()).unwrap();
        cs.finalize();

        let stats = Optimizer::from_cs(cs).stats();
        assert!(
            stats.num_constraints > 2000 && stats.num_constraints < 2500,
            "ItemExistsSMT: expected ~2180, got {}",
            stats.num_constraints
        );
    }

    // CapacitySMT
    {
        let cs = ConstraintSystem::<Fr>::new_ref();
        CapacitySMTCircuit::empty().generate_constraints(cs.clone()).unwrap();
        cs.finalize();

        let stats = Optimizer::from_cs(cs).stats();
        assert!(
            stats.num_constraints > 300 && stats.num_constraints < 500,
            "CapacitySMT: expected ~379, got {}",
            stats.num_constraints
        );
    }
}
