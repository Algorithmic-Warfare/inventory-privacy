//! Constant Folding Pass - Remove compile-time verifiable constraints.
//!
//! # Pattern
//! Constraints where A and B are both constants: `c1 * c2 = c3`
//!
//! # Reduction
//! Verify the constraint is satisfied, then remove it.
//! If not satisfied, report an error (constraint system is unsatisfiable).
//!
//! # Example
//! ```text
//! Before:
//!   Constraint 0: 5 * 3 = 15  ← constant, verifiable
//!   Constraint 1: x * y = z
//!
//! After verification (5 * 3 = 15 ✓):
//!   Constraint 0: x * y = z  ← reindexed
//! ```

use ark_ff::PrimeField;

use crate::constraint::ConstraintMatrix;
use crate::reduction::{MatchMetadata, PatternMatch, PatternType, ReductionPass};

/// Constant folding pass: removes constraints that can be verified at compile time.
pub struct ConstantFoldingPass;

impl ConstantFoldingPass {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ConstantFoldingPass {
    fn default() -> Self {
        Self::new()
    }
}

impl<F: PrimeField> ReductionPass<F> for ConstantFoldingPass {
    fn name(&self) -> &'static str {
        "Constant Folding"
    }

    fn description(&self) -> &'static str {
        "Removes constraints where all terms are constants (compile-time verifiable)"
    }

    fn scan(&self, matrix: &ConstraintMatrix<F>) -> Vec<PatternMatch> {
        let mut matches = Vec::new();
        let mut match_id = 0;

        for constraint in &matrix.constraints {
            if constraint.is_constant() {
                // Compute the constant values
                let a_val = compute_constant_value(&constraint.a);
                let b_val = compute_constant_value(&constraint.b);
                let c_val = compute_constant_value(&constraint.c);

                let product = a_val * b_val;
                let is_satisfied = product == c_val;

                matches.push(PatternMatch {
                    id: match_id,
                    pattern_type: PatternType::Constant,
                    constraint_indices: vec![constraint.index],
                    variable_indices: vec![],
                    estimated_reduction: if is_satisfied { 1 } else { 0 },
                    description: if is_satisfied {
                        format!(
                            "Constraint {} is constant and satisfied (can be removed)",
                            constraint.index
                        )
                    } else {
                        format!(
                            "Constraint {} is constant but UNSATISFIED (system is invalid!)",
                            constraint.index
                        )
                    },
                    metadata: MatchMetadata::default(),
                });
                match_id += 1;
            }
        }

        matches
    }

    fn reduce(&self, matrix: ConstraintMatrix<F>, matches: &[PatternMatch]) -> ConstraintMatrix<F> {
        if matches.is_empty() {
            return matrix;
        }

        // Only remove satisfied constant constraints
        let to_remove: Vec<usize> = matches
            .iter()
            .filter(|m| m.pattern_type == PatternType::Constant && m.estimated_reduction > 0)
            .flat_map(|m| m.constraint_indices.iter().copied())
            .collect();

        matrix.without_constraints(&to_remove)
    }
}

/// Compute the constant value of a linear combination (assuming it's constant)
fn compute_constant_value<F: PrimeField>(lc: &crate::constraint::LinearCombination<F>) -> F {
    let mut sum = F::zero();
    for term in &lc.terms {
        if term.variable == 0 {
            // Variable 0 is the constant "1"
            sum += term.coefficient;
        }
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_bn254::Fr;
    use ark_relations::r1cs::{
        ConstraintSynthesizer, ConstraintSystem, ConstraintSystemRef,
        LinearCombination as ArkLC, SynthesisError, Variable,
    };

    struct ConstantTestCircuit;

    impl ConstraintSynthesizer<Fr> for ConstantTestCircuit {
        fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
            let one = Variable::One;

            // Constant constraint: 5 * 3 = 15
            cs.enforce_constraint(
                ArkLC::zero() + (Fr::from(5u64), one),
                ArkLC::zero() + (Fr::from(3u64), one),
                ArkLC::zero() + (Fr::from(15u64), one),
            )?;

            // Non-constant constraint
            let x = cs.new_witness_variable(|| Ok(Fr::from(2u64)))?;
            let y = cs.new_witness_variable(|| Ok(Fr::from(4u64)))?;
            cs.enforce_constraint(ArkLC::from(x), ArkLC::from(x), ArkLC::from(y))?;

            Ok(())
        }
    }

    #[test]
    fn test_constant_folding_scan() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        ConstantTestCircuit.generate_constraints(cs.clone()).unwrap();
        cs.finalize();

        let matrix = ConstraintMatrix::from_cs(cs);
        let pass = ConstantFoldingPass::new();
        let matches = pass.scan(&matrix);

        assert_eq!(matches.len(), 1, "Should find 1 constant constraint");
        assert_eq!(matches[0].estimated_reduction, 1);
    }

    #[test]
    fn test_constant_folding_reduce() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        ConstantTestCircuit.generate_constraints(cs.clone()).unwrap();
        cs.finalize();

        let matrix = ConstraintMatrix::from_cs(cs);
        assert_eq!(matrix.num_constraints(), 2);

        let pass = ConstantFoldingPass::new();
        let (reduced, _) = pass.optimize(matrix);

        assert_eq!(reduced.num_constraints(), 1, "Should have 1 constraint after folding");
    }
}
