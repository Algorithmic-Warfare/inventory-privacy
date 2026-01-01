//! Integration tests for R1CS Optimizer using real circuits.

use ark_bn254::Fr;
use ark_ff::Field;
use ark_relations::r1cs::{
    ConstraintSynthesizer, ConstraintSystemRef, SynthesisError, ConstraintSystem,
    LinearCombination, Variable,
};
use r1cs_optimizer::{Optimizer, OptimizerConfig};

/// A simple example circuit for testing: x^3 + x + 5 = y
struct CubicCircuit {
    x: Option<Fr>,
    y: Option<Fr>,
}

impl ConstraintSynthesizer<Fr> for CubicCircuit {
    fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
        let x = cs.new_witness_variable(|| self.x.ok_or(SynthesisError::AssignmentMissing))?;

        let x_squared = cs.new_witness_variable(|| {
            let x_val = self.x.ok_or(SynthesisError::AssignmentMissing)?;
            Ok(x_val.square())
        })?;

        let x_cubed = cs.new_witness_variable(|| {
            let x_val = self.x.ok_or(SynthesisError::AssignmentMissing)?;
            Ok(x_val.square() * x_val)
        })?;

        let y = cs.new_input_variable(|| self.y.ok_or(SynthesisError::AssignmentMissing))?;

        // x * x = x^2
        cs.enforce_constraint(
            LinearCombination::from(x),
            LinearCombination::from(x),
            LinearCombination::from(x_squared),
        )?;

        // x^2 * x = x^3
        cs.enforce_constraint(
            LinearCombination::from(x_squared),
            LinearCombination::from(x),
            LinearCombination::from(x_cubed),
        )?;

        // 1 * (x^3 + x + 5) = y
        let one = Variable::One;
        let five = Fr::from(5u64);
        cs.enforce_constraint(
            LinearCombination::from(one),
            LinearCombination::from(x_cubed) + LinearCombination::from(x) + (five, one),
            LinearCombination::from(y),
        )?;

        Ok(())
    }
}

/// Circuit with duplicate constraints
struct DuplicateCircuit {
    x: Option<Fr>,
    x_squared: Option<Fr>,
}

impl ConstraintSynthesizer<Fr> for DuplicateCircuit {
    fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
        let x = cs.new_witness_variable(|| self.x.ok_or(SynthesisError::AssignmentMissing))?;
        // Make x_squared a public input so it's not eliminated as dead
        let x_squared = cs.new_input_variable(|| self.x_squared.ok_or(SynthesisError::AssignmentMissing))?;

        // Same constraint repeated 3 times
        for _ in 0..3 {
            cs.enforce_constraint(
                LinearCombination::from(x),
                LinearCombination::from(x),
                LinearCombination::from(x_squared),
            )?;
        }

        Ok(())
    }
}

/// Circuit with boolean constraints
struct BooleanCircuit {
    bits: Vec<Option<bool>>,
}

impl ConstraintSynthesizer<Fr> for BooleanCircuit {
    fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
        for bit_opt in &self.bits {
            let b = cs.new_witness_variable(|| {
                bit_opt
                    .map(|b| if b { Fr::from(1u64) } else { Fr::from(0u64) })
                    .ok_or(SynthesisError::AssignmentMissing)
            })?;

            // Boolean constraint: b * b = b
            cs.enforce_constraint(
                LinearCombination::from(b),
                LinearCombination::from(b),
                LinearCombination::from(b),
            )?;
        }

        Ok(())
    }
}

#[test]
fn test_optimizer_cubic_circuit() {
    let x = Fr::from(3u64);
    let y = x * x * x + x + Fr::from(5u64);

    let circuit = CubicCircuit {
        x: Some(x),
        y: Some(y),
    };

    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit.generate_constraints(cs.clone()).unwrap();
    cs.finalize();

    let optimizer = Optimizer::from_cs(cs);
    let stats = optimizer.stats();

    assert_eq!(stats.num_constraints, 3);
    assert!(stats.linear_constraints > 0);
}

#[test]
fn test_optimize_duplicates() {
    let x = Fr::from(5u64);
    let circuit = DuplicateCircuit {
        x: Some(x),
        x_squared: Some(x * x),
    };

    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit.generate_constraints(cs.clone()).unwrap();
    cs.finalize();

    let optimizer = Optimizer::from_cs(cs);
    let result = optimizer.optimize();

    // Should reduce from 3 to 1
    assert_eq!(result.original_constraints, 3);
    assert_eq!(result.final_constraints, 1);
    assert!(result.reduction_percentage() > 60.0);
}

#[test]
fn test_boolean_constraints() {
    let circuit = BooleanCircuit {
        bits: vec![Some(true), Some(false), Some(true)],
    };

    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit.generate_constraints(cs.clone()).unwrap();
    cs.finalize();

    let optimizer = Optimizer::from_cs(cs);
    let stats = optimizer.stats();

    assert_eq!(stats.num_constraints, 3);
    assert_eq!(stats.boolean_constraints, 3);
}

#[test]
fn test_safe_config() {
    let circuit = CubicCircuit {
        x: Some(Fr::from(2u64)),
        y: Some(Fr::from(15u64)),
    };

    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit.generate_constraints(cs.clone()).unwrap();
    cs.finalize();

    let optimizer = Optimizer::from_cs(cs)
        .with_config(OptimizerConfig::safe());

    let result = optimizer.optimize();
    // Safe config should still work
    assert!(result.original_constraints > 0);
}

#[test]
fn test_empty_circuit() {
    let cs = ConstraintSystem::<Fr>::new_ref();
    cs.finalize();

    let optimizer = Optimizer::from_cs(cs);
    let result = optimizer.optimize();

    assert_eq!(result.original_constraints, 0);
    assert_eq!(result.final_constraints, 0);
}

#[test]
fn test_analyze_only() {
    let x = Fr::from(5u64);
    let circuit = DuplicateCircuit {
        x: Some(x),
        x_squared: Some(x * x),
    };

    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit.generate_constraints(cs.clone()).unwrap();
    cs.finalize();

    let optimizer = Optimizer::from_cs(cs);
    let reports = optimizer.analyze();

    // Should produce reports about duplicates
    assert!(!reports.is_empty());
}
