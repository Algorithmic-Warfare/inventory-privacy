//! Dead Variable Pass - Remove unused variable definitions.
//!
//! # Pattern
//! Variables that are defined (appear on the C side of a constraint)
//! but never used anywhere else in the system.
//!
//! # Reduction
//! Remove constraints that only define dead variables.
//!
//! # Example
//! ```text
//! Before:
//!   Constraint 0: a * b = unused    ← 'unused' is never referenced elsewhere
//!   Constraint 1: x * y = z
//!   Constraint 2: z * 2 = output
//!
//! After:
//!   Constraint 0: x * y = z
//!   Constraint 1: z * 2 = output
//! ```
//!
//! # Note
//! This analysis traces from public outputs backward. Variables not reachable
//! from public outputs are considered dead.

use ark_ff::PrimeField;
use std::collections::{HashMap, HashSet};

use crate::constraint::ConstraintMatrix;
use crate::reduction::{MatchMetadata, PatternMatch, PatternType, ReductionPass};

/// Dead variable elimination pass.
pub struct DeadVariablePass;

impl DeadVariablePass {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DeadVariablePass {
    fn default() -> Self {
        Self::new()
    }
}

impl<F: PrimeField> ReductionPass<F> for DeadVariablePass {
    fn name(&self) -> &'static str {
        "Dead Variable Elimination"
    }

    fn description(&self) -> &'static str {
        "Removes constraints that define variables never used in outputs"
    }

    fn scan(&self, matrix: &ConstraintMatrix<F>) -> Vec<PatternMatch> {
        let mut matches = Vec::new();

        // Build usage map: for each variable, which constraints use it?
        let mut var_usage: HashMap<usize, HashSet<usize>> = HashMap::new();
        let mut var_definitions: HashMap<usize, Vec<usize>> = HashMap::new();

        for constraint in &matrix.constraints {
            // Variables used in A and B are "inputs" to this constraint
            for var in constraint.a.variables() {
                var_usage.entry(var).or_default().insert(constraint.index);
            }
            for var in constraint.b.variables() {
                var_usage.entry(var).or_default().insert(constraint.index);
            }

            // Variables in C that are single variables are "defined" by this constraint
            if constraint.c.is_single_variable() {
                let defined_var = constraint.c.terms[0].variable;
                var_definitions.entry(defined_var).or_default().push(constraint.index);
            }
        }

        // Find variables that are only defined once and never used
        let mut match_id = 0;
        for (var, defining_constraints) in &var_definitions {
            // Skip public inputs
            if *var < matrix.num_public_inputs {
                continue;
            }

            // Check if this variable is used anywhere (in A or B of any constraint)
            let usage_count = var_usage.get(var).map(|s| s.len()).unwrap_or(0);

            if usage_count == 0 && defining_constraints.len() == 1 {
                let constraint_idx = defining_constraints[0];

                matches.push(PatternMatch {
                    id: match_id,
                    pattern_type: PatternType::DeadVariable,
                    constraint_indices: vec![constraint_idx],
                    variable_indices: vec![*var],
                    estimated_reduction: 1,
                    description: format!(
                        "Variable {} is defined in constraint {} but never used",
                        var, constraint_idx
                    ),
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

        let to_remove: Vec<usize> = matches
            .iter()
            .filter(|m| m.pattern_type == PatternType::DeadVariable)
            .flat_map(|m| m.constraint_indices.iter().copied())
            .collect();

        matrix.without_constraints(&to_remove)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_bn254::Fr;
    use ark_relations::r1cs::{
        ConstraintSynthesizer, ConstraintSystem, ConstraintSystemRef,
        LinearCombination as ArkLC, SynthesisError,
    };

    struct DeadVarTestCircuit;

    impl ConstraintSynthesizer<Fr> for DeadVarTestCircuit {
        fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
            let a = cs.new_witness_variable(|| Ok(Fr::from(2u64)))?;
            let b = cs.new_witness_variable(|| Ok(Fr::from(3u64)))?;
            let dead = cs.new_witness_variable(|| Ok(Fr::from(6u64)))?;
            let used = cs.new_witness_variable(|| Ok(Fr::from(5u64)))?;
            let output = cs.new_input_variable(|| Ok(Fr::from(10u64)))?;

            // Dead: a * b = dead (dead is never used)
            cs.enforce_constraint(ArkLC::from(a), ArkLC::from(b), ArkLC::from(dead))?;

            // Used: a + b = used
            cs.enforce_constraint(
                ArkLC::from(ark_relations::r1cs::Variable::One),
                ArkLC::from(a) + ArkLC::from(b),
                ArkLC::from(used),
            )?;

            // Output: used * 2 = output
            cs.enforce_constraint(
                ArkLC::from(used),
                ArkLC::zero() + (Fr::from(2u64), ark_relations::r1cs::Variable::One),
                ArkLC::from(output),
            )?;

            Ok(())
        }
    }

    #[test]
    fn test_dead_variable_scan() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        DeadVarTestCircuit.generate_constraints(cs.clone()).unwrap();
        cs.finalize();

        let matrix = ConstraintMatrix::from_cs(cs);
        let pass = DeadVariablePass::new();
        let matches = pass.scan(&matrix);

        // Should find 'dead' variable
        assert!(!matches.is_empty(), "Should find dead variable");
    }

    #[test]
    fn test_dead_variable_reduce() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        DeadVarTestCircuit.generate_constraints(cs.clone()).unwrap();
        cs.finalize();

        let matrix = ConstraintMatrix::from_cs(cs);
        assert_eq!(matrix.num_constraints(), 3);

        let pass = DeadVariablePass::new();
        let (reduced, report) = pass.optimize(matrix);

        // Should remove the constraint defining 'dead'
        assert!(reduced.num_constraints() < 3, "Should remove dead constraint");
        assert!(report.estimated_savings > 0);
    }
}
