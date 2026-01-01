//! Deduplication Pass - Remove duplicate constraints.
//!
//! # Pattern
//! Constraints that are structurally identical (same A, B, C linear combinations).
//!
//! # Reduction
//! Keep one copy, remove all duplicates.
//!
//! # Example
//! ```text
//! Before:
//!   Constraint 0: x * y = z
//!   Constraint 1: x * y = z  ← duplicate
//!   Constraint 2: x * y = z  ← duplicate
//!   Constraint 3: a * b = c
//!
//! After:
//!   Constraint 0: x * y = z  ← kept (canonical)
//!   Constraint 1: a * b = c  ← reindexed
//! ```

use ark_ff::PrimeField;
use std::collections::HashMap;

use crate::constraint::{Constraint, ConstraintMatrix, LinearCombination};
use crate::reduction::{PatternMatch, PatternType, MatchMetadata, ReductionPass};

/// Deduplication pass: removes duplicate constraints.
pub struct DeduplicationPass;

impl DeduplicationPass {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DeduplicationPass {
    fn default() -> Self {
        Self::new()
    }
}

impl<F: PrimeField> ReductionPass<F> for DeduplicationPass {
    fn name(&self) -> &'static str {
        "Deduplication"
    }

    fn description(&self) -> &'static str {
        "Removes duplicate constraints that are structurally identical"
    }

    fn scan(&self, matrix: &ConstraintMatrix<F>) -> Vec<PatternMatch> {
        let mut matches = Vec::new();
        let mut hash_to_constraints: HashMap<u64, Vec<usize>> = HashMap::new();

        // Group constraints by hash
        for constraint in &matrix.constraints {
            let hash = constraint.constraint_hash();
            hash_to_constraints
                .entry(hash)
                .or_default()
                .push(constraint.index);
        }

        // Find groups with duplicates
        let mut match_id = 0;
        for (hash, indices) in hash_to_constraints {
            if indices.len() > 1 {
                // Verify they're actually equal (hash collision check)
                let first = &matrix.constraints[indices[0]];
                let mut confirmed_duplicates = vec![indices[0]];

                for &idx in &indices[1..] {
                    let other = &matrix.constraints[idx];
                    if constraints_equal(first, other) {
                        confirmed_duplicates.push(idx);
                    }
                }

                if confirmed_duplicates.len() > 1 {
                    let canonical = confirmed_duplicates[0];
                    let duplicates: Vec<usize> = confirmed_duplicates[1..].to_vec();
                    let removable = duplicates.len();

                    matches.push(PatternMatch {
                        id: match_id,
                        pattern_type: PatternType::Duplicate,
                        constraint_indices: confirmed_duplicates.clone(),
                        variable_indices: vec![],
                        estimated_reduction: removable,
                        description: format!(
                            "Constraint {} has {} duplicate(s): {:?}",
                            canonical, removable, duplicates
                        ),
                        metadata: MatchMetadata {
                            canonical_index: Some(canonical),
                            expression_hash: Some(hash),
                            ..Default::default()
                        },
                    });
                    match_id += 1;
                }
            }
        }

        matches
    }

    fn reduce(&self, matrix: ConstraintMatrix<F>, matches: &[PatternMatch]) -> ConstraintMatrix<F> {
        if matches.is_empty() {
            return matrix;
        }

        // Collect all constraint indices to remove (duplicates, not canonicals)
        let mut to_remove: Vec<usize> = Vec::new();

        for m in matches {
            if m.pattern_type == PatternType::Duplicate {
                // Keep the canonical (first), remove the rest
                if let Some(canonical) = m.metadata.canonical_index {
                    for &idx in &m.constraint_indices {
                        if idx != canonical {
                            to_remove.push(idx);
                        }
                    }
                }
            }
        }

        // Remove duplicates from the list itself
        to_remove.sort();
        to_remove.dedup();

        // Create new matrix without duplicates
        matrix.without_constraints(&to_remove)
    }
}

/// Check if two constraints are structurally equal
fn constraints_equal<F: PrimeField>(a: &Constraint<F>, b: &Constraint<F>) -> bool {
    lc_equal(&a.a, &b.a) && lc_equal(&a.b, &b.b) && lc_equal(&a.c, &b.c)
}

/// Check if two linear combinations are equal
fn lc_equal<F: PrimeField>(a: &LinearCombination<F>, b: &LinearCombination<F>) -> bool {
    if a.terms.len() != b.terms.len() {
        return false;
    }

    // Build map for comparison
    let mut a_map: HashMap<usize, F> = HashMap::new();
    for term in &a.terms {
        if !term.coefficient.is_zero() {
            a_map.insert(term.variable, term.coefficient);
        }
    }

    for term in &b.terms {
        if term.coefficient.is_zero() {
            continue;
        }
        match a_map.get(&term.variable) {
            Some(coeff) if *coeff == term.coefficient => {}
            _ => return false,
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_bn254::Fr;
    use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystem, ConstraintSystemRef, LinearCombination as ArkLC, SynthesisError};

    struct DuplicateTestCircuit;

    impl ConstraintSynthesizer<Fr> for DuplicateTestCircuit {
        fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
            let x = cs.new_witness_variable(|| Ok(Fr::from(5u64)))?;
            let y = cs.new_witness_variable(|| Ok(Fr::from(3u64)))?;
            let z = cs.new_witness_variable(|| Ok(Fr::from(15u64)))?;

            // Original
            cs.enforce_constraint(ArkLC::from(x), ArkLC::from(y), ArkLC::from(z))?;
            // Duplicate 1
            cs.enforce_constraint(ArkLC::from(x), ArkLC::from(y), ArkLC::from(z))?;
            // Duplicate 2
            cs.enforce_constraint(ArkLC::from(x), ArkLC::from(y), ArkLC::from(z))?;
            // Different constraint
            let w = cs.new_witness_variable(|| Ok(Fr::from(25u64)))?;
            cs.enforce_constraint(ArkLC::from(x), ArkLC::from(x), ArkLC::from(w))?;

            Ok(())
        }
    }

    #[test]
    fn test_deduplication_scan() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        DuplicateTestCircuit.generate_constraints(cs.clone()).unwrap();
        cs.finalize();

        let matrix = ConstraintMatrix::from_cs(cs);
        let pass = DeduplicationPass::new();
        let matches = pass.scan(&matrix);

        assert_eq!(matches.len(), 1, "Should find 1 duplicate group");
        assert_eq!(matches[0].estimated_reduction, 2, "Should save 2 constraints");
    }

    #[test]
    fn test_deduplication_reduce() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        DuplicateTestCircuit.generate_constraints(cs.clone()).unwrap();
        cs.finalize();

        let matrix = ConstraintMatrix::from_cs(cs);
        assert_eq!(matrix.num_constraints(), 4);

        let pass = DeduplicationPass::new();
        let (reduced, report) = pass.optimize(matrix);

        assert_eq!(reduced.num_constraints(), 2, "Should have 2 constraints after dedup");
        assert_eq!(report.estimated_savings, 2);
    }
}
