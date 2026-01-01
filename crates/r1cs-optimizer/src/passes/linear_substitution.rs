//! Linear Substitution Pass - Inline linear constraint definitions.
//!
//! # Pattern
//! Linear constraints of the form `1 * expr = var` or `expr * 1 = var`
//! where `var` is a single variable.
//!
//! # Reduction
//! Substitute `var` with `expr` throughout all other constraints,
//! then remove the defining constraint.
//!
//! # Example
//! ```text
//! Before:
//!   Constraint 0: 1 * (a + b) = sum    ← linear definition
//!   Constraint 1: sum * x = result     ← uses 'sum'
//!
//! After substituting sum = (a + b):
//!   Constraint 0: (a + b) * x = result ← inlined
//! ```
//!
//! # Note
//! This pass is conservative - it only substitutes simple cases to avoid
//! constraint explosion from inlining complex expressions.

use ark_ff::PrimeField;
use std::collections::HashMap;

use crate::constraint::{Constraint, ConstraintMatrix, LinearCombination, Term};
use crate::reduction::{MatchMetadata, PatternMatch, PatternType, ReductionPass};

/// Maximum terms in an expression to consider for substitution.
/// Larger expressions may cause constraint explosion when inlined.
const MAX_SUBSTITUTION_TERMS: usize = 4;

/// Linear substitution pass: inlines simple variable definitions.
pub struct LinearSubstitutionPass {
    max_terms: usize,
}

impl LinearSubstitutionPass {
    pub fn new() -> Self {
        Self {
            max_terms: MAX_SUBSTITUTION_TERMS,
        }
    }

    /// Set maximum terms allowed for substitution.
    pub fn with_max_terms(mut self, max: usize) -> Self {
        self.max_terms = max;
        self
    }
}

impl Default for LinearSubstitutionPass {
    fn default() -> Self {
        Self::new()
    }
}

impl<F: PrimeField> ReductionPass<F> for LinearSubstitutionPass {
    fn name(&self) -> &'static str {
        "Linear Substitution"
    }

    fn description(&self) -> &'static str {
        "Inlines simple variable definitions to eliminate constraints"
    }

    fn scan(&self, matrix: &ConstraintMatrix<F>) -> Vec<PatternMatch> {
        let mut matches = Vec::new();
        let mut match_id = 0;

        for constraint in &matrix.constraints {
            if !constraint.is_linear() {
                continue;
            }

            // Get the expression and result variable
            let (expr, result) = if constraint.a.is_one() {
                (&constraint.b, &constraint.c)
            } else if constraint.b.is_one() {
                (&constraint.a, &constraint.c)
            } else {
                continue;
            };

            // Check if result is a single variable
            if !result.is_single_variable() {
                continue;
            }

            // Check if expression is simple enough
            if expr.num_terms() > self.max_terms {
                continue;
            }

            let target_var = result.terms[0].variable;

            // Don't substitute public inputs
            if target_var < matrix.num_public_inputs {
                continue;
            }

            // Count how many other constraints use this variable
            let usage_count = count_variable_usage(matrix, target_var, constraint.index);

            // Only worth substituting if the variable is used elsewhere
            if usage_count == 0 {
                continue;
            }

            matches.push(PatternMatch {
                id: match_id,
                pattern_type: PatternType::LinearSubstitution,
                constraint_indices: vec![constraint.index],
                variable_indices: vec![target_var],
                estimated_reduction: 1, // Remove the defining constraint
                description: format!(
                    "Variable {} defined by {} terms, used {} times",
                    target_var,
                    expr.num_terms(),
                    usage_count
                ),
                metadata: MatchMetadata {
                    substitute_variable: Some(target_var),
                    ..Default::default()
                },
            });
            match_id += 1;
        }

        matches
    }

    fn reduce(&self, matrix: ConstraintMatrix<F>, matches: &[PatternMatch]) -> ConstraintMatrix<F> {
        if matches.is_empty() {
            return matrix;
        }

        // Build substitution map: variable -> (expression LC, defining constraint)
        let mut substitutions: HashMap<usize, (LinearCombination<F>, usize)> = HashMap::new();

        for m in matches {
            if m.pattern_type != PatternType::LinearSubstitution {
                continue;
            }

            if let Some(var) = m.metadata.substitute_variable {
                let defining_idx = m.constraint_indices[0];
                let constraint = &matrix.constraints[defining_idx];

                // Extract the expression (the side that's not the result)
                let expr = if constraint.a.is_one() {
                    constraint.b.clone()
                } else {
                    constraint.a.clone()
                };

                substitutions.insert(var, (expr, defining_idx));
            }
        }

        // Apply substitutions to all constraints
        let mut new_constraints = Vec::new();
        let constraints_to_remove: Vec<usize> = substitutions.values()
            .map(|(_, idx)| *idx)
            .collect();

        for constraint in &matrix.constraints {
            if constraints_to_remove.contains(&constraint.index) {
                continue; // Skip the defining constraints
            }

            // Apply substitutions to each linear combination
            let new_a = apply_substitutions(&constraint.a, &substitutions);
            let new_b = apply_substitutions(&constraint.b, &substitutions);
            let new_c = apply_substitutions(&constraint.c, &substitutions);

            new_constraints.push(Constraint::new(
                new_constraints.len(),
                new_a,
                new_b,
                new_c,
            ));
        }

        ConstraintMatrix {
            constraints: new_constraints,
            num_public_inputs: matrix.num_public_inputs,
            num_private_witnesses: matrix.num_private_witnesses,
            num_variables: matrix.num_variables,
        }
    }
}

/// Count how many constraints use a variable (excluding the defining constraint)
fn count_variable_usage<F: PrimeField>(
    matrix: &ConstraintMatrix<F>,
    var: usize,
    exclude_idx: usize,
) -> usize {
    matrix
        .constraints
        .iter()
        .filter(|c| c.index != exclude_idx)
        .filter(|c| c.variables().contains(&var))
        .count()
}

/// Apply substitutions to a linear combination
fn apply_substitutions<F: PrimeField>(
    lc: &LinearCombination<F>,
    substitutions: &HashMap<usize, (LinearCombination<F>, usize)>,
) -> LinearCombination<F> {
    let mut new_terms: HashMap<usize, F> = HashMap::new();

    for term in &lc.terms {
        if let Some((replacement, _)) = substitutions.get(&term.variable) {
            // Substitute: add each term of the replacement scaled by coefficient
            for rep_term in &replacement.terms {
                *new_terms.entry(rep_term.variable).or_insert(F::zero()) +=
                    term.coefficient * rep_term.coefficient;
            }
        } else {
            // Keep original term
            *new_terms.entry(term.variable).or_insert(F::zero()) += term.coefficient;
        }
    }

    // Convert back to terms, removing zeros
    let terms: Vec<Term<F>> = new_terms
        .into_iter()
        .filter(|(_, coeff)| !coeff.is_zero())
        .map(|(var, coeff)| Term::new(var, coeff))
        .collect();

    LinearCombination::new(terms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_bn254::Fr;
    use ark_relations::r1cs::{
        ConstraintSynthesizer, ConstraintSystem, ConstraintSystemRef,
        LinearCombination as ArkLC, SynthesisError, Variable,
    };

    struct LinearSubstTestCircuit;

    impl ConstraintSynthesizer<Fr> for LinearSubstTestCircuit {
        fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
            let a = cs.new_witness_variable(|| Ok(Fr::from(2u64)))?;
            let b = cs.new_witness_variable(|| Ok(Fr::from(3u64)))?;
            let sum = cs.new_witness_variable(|| Ok(Fr::from(5u64)))?;
            let result = cs.new_witness_variable(|| Ok(Fr::from(10u64)))?;

            // Linear: 1 * (a + b) = sum
            cs.enforce_constraint(
                ArkLC::from(Variable::One),
                ArkLC::from(a) + ArkLC::from(b),
                ArkLC::from(sum),
            )?;

            // Use sum: sum * 2 = result
            cs.enforce_constraint(
                ArkLC::from(sum),
                ArkLC::zero() + (Fr::from(2u64), Variable::One),
                ArkLC::from(result),
            )?;

            Ok(())
        }
    }

    #[test]
    fn test_linear_substitution_scan() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        LinearSubstTestCircuit.generate_constraints(cs.clone()).unwrap();
        cs.finalize();

        let matrix = ConstraintMatrix::from_cs(cs);
        let pass = LinearSubstitutionPass::new();
        let matches = pass.scan(&matrix);

        assert!(!matches.is_empty(), "Should find substitution opportunity");
    }

    #[test]
    fn test_linear_substitution_reduce() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        LinearSubstTestCircuit.generate_constraints(cs.clone()).unwrap();
        cs.finalize();

        let matrix = ConstraintMatrix::from_cs(cs);
        assert_eq!(matrix.num_constraints(), 2);

        let pass = LinearSubstitutionPass::new();
        let (reduced, report) = pass.optimize(matrix);

        assert_eq!(reduced.num_constraints(), 1, "Should have 1 constraint after substitution");
        assert_eq!(report.estimated_savings, 1);
    }
}
