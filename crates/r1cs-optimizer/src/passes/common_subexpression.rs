//! Common Subexpression Pass - Factor out repeated computations.
//!
//! # Pattern
//! Linear combinations that appear multiple times across different constraints.
//!
//! # Reduction
//! Introduce a new intermediate variable for the common expression,
//! then replace all occurrences with that variable.
//!
//! # Example
//! ```text
//! Before:
//!   Constraint 0: (a + b + c) * x = r1
//!   Constraint 1: (a + b + c) * y = r2  ← same (a + b + c)
//!   Constraint 2: (a + b + c) * z = r3  ← same (a + b + c)
//!
//! After introducing t = a + b + c:
//!   Constraint 0: 1 * (a + b + c) = t   ← new definition
//!   Constraint 1: t * x = r1
//!   Constraint 2: t * y = r2
//!   Constraint 3: t * z = r3
//! ```
//!
//! # Note
//! This pass is informational only in the current implementation.
//! Full CSE reduction requires careful variable management.

use ark_ff::PrimeField;
use std::collections::HashMap;

use crate::constraint::ConstraintMatrix;
use crate::reduction::{MatchMetadata, PatternMatch, PatternType, ReductionPass, ReductionReport};

/// Minimum occurrences to consider as common subexpression.
const MIN_OCCURRENCES: usize = 3;

/// Common subexpression detection pass.
pub struct CommonSubexpressionPass {
    min_occurrences: usize,
}

impl CommonSubexpressionPass {
    pub fn new() -> Self {
        Self {
            min_occurrences: MIN_OCCURRENCES,
        }
    }

    /// Set minimum occurrences threshold.
    pub fn with_min_occurrences(mut self, min: usize) -> Self {
        self.min_occurrences = min;
        self
    }
}

impl Default for CommonSubexpressionPass {
    fn default() -> Self {
        Self::new()
    }
}

impl<F: PrimeField> ReductionPass<F> for CommonSubexpressionPass {
    fn name(&self) -> &'static str {
        "Common Subexpression"
    }

    fn description(&self) -> &'static str {
        "Identifies repeated linear combinations that could be factored out"
    }

    fn scan(&self, matrix: &ConstraintMatrix<F>) -> Vec<PatternMatch> {
        let mut matches = Vec::new();

        // Track structural patterns by hash
        // Maps hash -> (constraint_idx, which_side: 'A'/'B'/'C')
        let mut lc_patterns: HashMap<u64, Vec<(usize, char)>> = HashMap::new();

        for constraint in &matrix.constraints {
            // Only consider non-trivial linear combinations (2+ terms)
            if constraint.a.num_terms() >= 2 {
                let hash = constraint.a.structural_hash();
                lc_patterns.entry(hash).or_default().push((constraint.index, 'A'));
            }
            if constraint.b.num_terms() >= 2 {
                let hash = constraint.b.structural_hash();
                lc_patterns.entry(hash).or_default().push((constraint.index, 'B'));
            }
            // C side is usually the output, less likely to be common
            if constraint.c.num_terms() >= 2 {
                let hash = constraint.c.structural_hash();
                lc_patterns.entry(hash).or_default().push((constraint.index, 'C'));
            }
        }

        // Report patterns that appear multiple times
        let mut match_id = 0;
        for (hash, occurrences) in lc_patterns {
            if occurrences.len() >= self.min_occurrences {
                let constraint_indices: Vec<usize> =
                    occurrences.iter().map(|(idx, _)| *idx).collect();

                let positions: Vec<String> = occurrences
                    .iter()
                    .take(5)
                    .map(|(idx, side)| format!("{}:{}", idx, side))
                    .collect();

                // Estimated savings: N occurrences -> 1 definition + N references
                // Net change: from N terms each time to 1 term each time
                // Actually adds 1 constraint but simplifies N constraints
                let estimated_savings = occurrences.len().saturating_sub(2);

                matches.push(PatternMatch {
                    id: match_id,
                    pattern_type: PatternType::CommonSubexpression,
                    constraint_indices,
                    variable_indices: vec![],
                    estimated_reduction: estimated_savings,
                    description: format!(
                        "Linear combination appears {} times at: {}{}",
                        occurrences.len(),
                        positions.join(", "),
                        if occurrences.len() > 5 { "..." } else { "" }
                    ),
                    metadata: MatchMetadata {
                        expression_hash: Some(hash),
                        ..Default::default()
                    },
                });
                match_id += 1;
            }
        }

        // Sort by estimated savings (descending)
        matches.sort_by(|a, b| b.estimated_reduction.cmp(&a.estimated_reduction));

        matches
    }

    fn reduce(&self, matrix: ConstraintMatrix<F>, _matches: &[PatternMatch]) -> ConstraintMatrix<F> {
        // CSE reduction is complex and requires:
        // 1. Allocating new intermediate variables
        // 2. Adding defining constraints
        // 3. Rewriting all occurrences
        //
        // For now, we only report opportunities.
        // Full implementation would need variable allocation hooks.
        matrix
    }

    fn report(&self, matches: &[PatternMatch]) -> ReductionReport {
        let mut report = ReductionReport::new("Common Subexpression");
        report.patterns_found = matches.len();

        for m in matches {
            report.reducible_constraints += m.constraint_indices.len();
            report.estimated_savings += m.estimated_reduction;
            report.add_finding(m.description.clone());
        }

        // Add note about manual implementation
        if !matches.is_empty() {
            report.add_finding(
                "Note: CSE reduction requires manual circuit refactoring".to_string()
            );
        }

        report
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

    struct CSETestCircuit;

    impl ConstraintSynthesizer<Fr> for CSETestCircuit {
        fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
            let a = cs.new_witness_variable(|| Ok(Fr::from(1u64)))?;
            let b = cs.new_witness_variable(|| Ok(Fr::from(2u64)))?;
            let x = cs.new_witness_variable(|| Ok(Fr::from(3u64)))?;
            let y = cs.new_witness_variable(|| Ok(Fr::from(4u64)))?;
            let z = cs.new_witness_variable(|| Ok(Fr::from(5u64)))?;
            let r1 = cs.new_witness_variable(|| Ok(Fr::from(9u64)))?;
            let r2 = cs.new_witness_variable(|| Ok(Fr::from(12u64)))?;
            let r3 = cs.new_witness_variable(|| Ok(Fr::from(15u64)))?;

            // Common expression (a + b) appears 3 times
            let common = ArkLC::from(a) + ArkLC::from(b);

            cs.enforce_constraint(common.clone(), ArkLC::from(x), ArkLC::from(r1))?;
            cs.enforce_constraint(common.clone(), ArkLC::from(y), ArkLC::from(r2))?;
            cs.enforce_constraint(common, ArkLC::from(z), ArkLC::from(r3))?;

            Ok(())
        }
    }

    #[test]
    fn test_cse_scan() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        CSETestCircuit.generate_constraints(cs.clone()).unwrap();
        cs.finalize();

        let matrix = ConstraintMatrix::from_cs(cs);
        let pass = CommonSubexpressionPass::new();
        let matches = pass.scan(&matrix);

        assert!(!matches.is_empty(), "Should find common subexpression");
    }
}
