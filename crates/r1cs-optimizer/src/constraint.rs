//! Constraint representation for analysis.
//!
//! This module provides a normalized representation of R1CS constraints
//! that's suitable for static analysis.

use ark_ff::PrimeField;
use ark_relations::r1cs::{
    ConstraintSystemRef, ConstraintMatrices, Matrix as ArkMatrix,
};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// A term in a linear combination: coefficient * variable
#[derive(Clone, Debug)]
pub struct Term<F: PrimeField> {
    /// Variable index (0 = constant 1, 1+ = variables)
    pub variable: usize,
    /// Coefficient
    pub coefficient: F,
}

impl<F: PrimeField> Term<F> {
    pub fn new(variable: usize, coefficient: F) -> Self {
        Self { variable, coefficient }
    }

    /// Check if this is the constant term (variable index 0)
    pub fn is_constant(&self) -> bool {
        self.variable == 0
    }
}

/// A linear combination of variables: sum of (coefficient * variable)
#[derive(Clone, Debug)]
pub struct LinearCombination<F: PrimeField> {
    /// Terms in the linear combination
    pub terms: Vec<Term<F>>,
}

impl<F: PrimeField> LinearCombination<F> {
    pub fn new(terms: Vec<Term<F>>) -> Self {
        Self { terms }
    }

    pub fn empty() -> Self {
        Self { terms: Vec::new() }
    }

    /// Check if this is just a constant (only the constant term)
    pub fn is_constant(&self) -> bool {
        self.terms.iter().all(|t| t.is_constant())
    }

    /// Check if this is just a single variable (coeff * var)
    pub fn is_single_variable(&self) -> bool {
        self.terms.len() == 1 && !self.terms[0].is_constant()
    }

    /// Check if this equals the constant 1
    pub fn is_one(&self) -> bool {
        self.terms.len() == 1
            && self.terms[0].is_constant()
            && self.terms[0].coefficient == F::one()
    }

    /// Check if this is zero (empty or all zero coefficients)
    pub fn is_zero(&self) -> bool {
        self.terms.is_empty() || self.terms.iter().all(|t| t.coefficient.is_zero())
    }

    /// Get the number of non-zero terms
    pub fn num_terms(&self) -> usize {
        self.terms.iter().filter(|t| !t.coefficient.is_zero()).count()
    }

    /// Get variable indices used in this linear combination
    pub fn variables(&self) -> Vec<usize> {
        self.terms.iter()
            .filter(|t| !t.is_constant() && !t.coefficient.is_zero())
            .map(|t| t.variable)
            .collect()
    }

    /// Compute a hash for comparison (ignoring coefficient values, just structure)
    pub fn structural_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();

        // Sort by variable index for canonical form
        let mut sorted_vars: Vec<_> = self.terms.iter()
            .filter(|t| !t.coefficient.is_zero())
            .map(|t| t.variable)
            .collect();
        sorted_vars.sort();

        sorted_vars.hash(&mut hasher);
        hasher.finish()
    }

    /// Compute a full hash including coefficients
    pub fn full_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();

        // Sort by variable index for canonical form
        let mut sorted: Vec<_> = self.terms.iter()
            .filter(|t| !t.coefficient.is_zero())
            .map(|t| {
                // Use bigint representation for hashing
                let bigint = t.coefficient.into_bigint();
                (t.variable, bigint)
            })
            .collect();
        sorted.sort_by_key(|(v, _)| *v);

        for (var, coeff) in sorted {
            var.hash(&mut hasher);
            // Hash the limbs of the big integer
            coeff.as_ref().hash(&mut hasher);
        }
        hasher.finish()
    }
}

/// A single R1CS constraint: A * B = C
/// where A, B, C are linear combinations of variables.
#[derive(Clone, Debug)]
pub struct Constraint<F: PrimeField> {
    /// Index of this constraint
    pub index: usize,
    /// Left side of multiplication
    pub a: LinearCombination<F>,
    /// Right side of multiplication
    pub b: LinearCombination<F>,
    /// Result (must equal A * B)
    pub c: LinearCombination<F>,
}

impl<F: PrimeField> Constraint<F> {
    pub fn new(
        index: usize,
        a: LinearCombination<F>,
        b: LinearCombination<F>,
        c: LinearCombination<F>,
    ) -> Self {
        Self { index, a, b, c }
    }

    /// Check if this is a linear constraint (A or B equals 1)
    pub fn is_linear(&self) -> bool {
        self.a.is_one() || self.b.is_one()
    }

    /// Check if this is a constant constraint (A and B are constants)
    pub fn is_constant(&self) -> bool {
        self.a.is_constant() && self.b.is_constant()
    }

    /// Check if this is a boolean constraint (v * v = v or v * (1-v) = 0)
    pub fn is_boolean(&self) -> bool {
        // v * v = v pattern
        if self.a.is_single_variable() && self.b.is_single_variable() && self.c.is_single_variable() {
            let a_var = self.a.terms[0].variable;
            let b_var = self.b.terms[0].variable;
            let c_var = self.c.terms[0].variable;
            if a_var == b_var && b_var == c_var {
                return true;
            }
        }
        false
    }

    /// Compute a hash for duplicate detection
    pub fn constraint_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();

        self.a.full_hash().hash(&mut hasher);
        self.b.full_hash().hash(&mut hasher);
        self.c.full_hash().hash(&mut hasher);

        hasher.finish()
    }

    /// Get all variables used in this constraint
    pub fn variables(&self) -> Vec<usize> {
        let mut vars = self.a.variables();
        vars.extend(self.b.variables());
        vars.extend(self.c.variables());
        vars.sort();
        vars.dedup();
        vars
    }

    /// Total number of non-zero terms across A, B, C
    pub fn num_terms(&self) -> usize {
        self.a.num_terms() + self.b.num_terms() + self.c.num_terms()
    }
}

/// Complete constraint matrix representation
#[derive(Clone, Debug)]
pub struct ConstraintMatrix<F: PrimeField> {
    /// All constraints
    pub constraints: Vec<Constraint<F>>,
    /// Number of public inputs
    pub num_public_inputs: usize,
    /// Number of private witnesses
    pub num_private_witnesses: usize,
    /// Total variables (1 + public + private, where 1 is the constant)
    pub num_variables: usize,
}

impl<F: PrimeField> ConstraintMatrix<F> {
    /// Extract constraint matrix from arkworks ConstraintSystemRef
    pub fn from_cs(cs: ConstraintSystemRef<F>) -> Self {
        let matrices = cs.to_matrices().expect("Failed to get constraint matrices");
        Self::from_matrices(&matrices, cs.num_instance_variables(), cs.num_witness_variables())
    }

    /// Build from arkworks matrices
    fn from_matrices(
        matrices: &ConstraintMatrices<F>,
        num_instance: usize,
        num_witness: usize,
    ) -> Self {
        let num_constraints = matrices.num_constraints;
        let num_variables = matrices.num_instance_variables + matrices.num_witness_variables;

        let mut constraints = Vec::with_capacity(num_constraints);

        for i in 0..num_constraints {
            let a = Self::extract_lc(&matrices.a, i, num_variables);
            let b = Self::extract_lc(&matrices.b, i, num_variables);
            let c = Self::extract_lc(&matrices.c, i, num_variables);

            constraints.push(Constraint::new(i, a, b, c));
        }

        Self {
            constraints,
            num_public_inputs: num_instance,
            num_private_witnesses: num_witness,
            num_variables,
        }
    }

    /// Extract a linear combination for a specific constraint row
    fn extract_lc(
        matrix: &ArkMatrix<F>,
        row: usize,
        _num_vars: usize,
    ) -> LinearCombination<F> {
        let terms: Vec<Term<F>> = matrix[row]
            .iter()
            .map(|(coeff, var)| Term::new(*var, *coeff))
            .collect();

        LinearCombination::new(terms)
    }

    /// Create an empty constraint matrix with the given dimensions.
    pub fn empty(num_public_inputs: usize, num_private_witnesses: usize) -> Self {
        Self {
            constraints: Vec::new(),
            num_public_inputs,
            num_private_witnesses,
            num_variables: num_public_inputs + num_private_witnesses,
        }
    }

    /// Create a new matrix with only the specified constraint indices.
    pub fn with_constraints(&self, indices: &[usize]) -> Self {
        let mut new_constraints = Vec::with_capacity(indices.len());

        for (new_idx, &old_idx) in indices.iter().enumerate() {
            if old_idx < self.constraints.len() {
                let old_constraint = &self.constraints[old_idx];
                new_constraints.push(Constraint::new(
                    new_idx,
                    old_constraint.a.clone(),
                    old_constraint.b.clone(),
                    old_constraint.c.clone(),
                ));
            }
        }

        Self {
            constraints: new_constraints,
            num_public_inputs: self.num_public_inputs,
            num_private_witnesses: self.num_private_witnesses,
            num_variables: self.num_variables,
        }
    }

    /// Create a new matrix excluding the specified constraint indices.
    pub fn without_constraints(&self, indices_to_remove: &[usize]) -> Self {
        let remove_set: std::collections::HashSet<_> = indices_to_remove.iter().collect();

        let keep_indices: Vec<usize> = (0..self.constraints.len())
            .filter(|i| !remove_set.contains(i))
            .collect();

        self.with_constraints(&keep_indices)
    }

    /// Add a constraint to the matrix.
    pub fn add_constraint(&mut self, a: LinearCombination<F>, b: LinearCombination<F>, c: LinearCombination<F>) {
        let index = self.constraints.len();
        self.constraints.push(Constraint::new(index, a, b, c));
    }

    /// Get total number of constraints
    pub fn num_constraints(&self) -> usize {
        self.constraints.len()
    }

    /// Get statistics about matrix sparsity
    pub fn sparsity_stats(&self) -> SparsityStats {
        let total_possible = self.num_constraints() * self.num_variables * 3; // A, B, C
        let total_nonzero: usize = self.constraints.iter()
            .map(|c| c.num_terms())
            .sum();

        let density = if total_possible > 0 {
            total_nonzero as f64 / total_possible as f64
        } else {
            0.0
        };

        let avg_terms = if self.num_constraints() > 0 {
            total_nonzero as f64 / self.num_constraints() as f64
        } else {
            0.0
        };

        // Count variable frequency
        let mut var_frequency: HashMap<usize, usize> = HashMap::new();
        for constraint in &self.constraints {
            for var in constraint.variables() {
                *var_frequency.entry(var).or_insert(0) += 1;
            }
        }

        let max_frequency = var_frequency.values().max().copied().unwrap_or(0);
        let hot_variables: Vec<(usize, usize)> = var_frequency.into_iter()
            .filter(|(_, freq)| *freq > self.num_constraints() / 20) // > 5% of constraints
            .collect();

        SparsityStats {
            total_constraints: self.num_constraints(),
            total_variables: self.num_variables,
            total_nonzero_terms: total_nonzero,
            density,
            avg_terms_per_constraint: avg_terms,
            max_variable_frequency: max_frequency,
            hot_variables,
        }
    }
}

/// Statistics about matrix sparsity
#[derive(Clone, Debug)]
pub struct SparsityStats {
    pub total_constraints: usize,
    pub total_variables: usize,
    pub total_nonzero_terms: usize,
    pub density: f64,
    pub avg_terms_per_constraint: f64,
    pub max_variable_frequency: usize,
    pub hot_variables: Vec<(usize, usize)>, // (variable_idx, frequency)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_bn254::Fr;

    #[test]
    fn test_linear_combination_is_one() {
        let lc: LinearCombination<Fr> = LinearCombination::new(vec![
            Term::new(0, Fr::from(1u64)),
        ]);
        assert!(lc.is_one());
        assert!(lc.is_constant());
    }

    #[test]
    fn test_linear_combination_structural_hash() {
        let lc1: LinearCombination<Fr> = LinearCombination::new(vec![
            Term::new(1, Fr::from(5u64)),
            Term::new(2, Fr::from(3u64)),
        ]);
        let lc2: LinearCombination<Fr> = LinearCombination::new(vec![
            Term::new(2, Fr::from(3u64)),
            Term::new(1, Fr::from(5u64)),
        ]);
        // Same variables, different order - should have same structural hash
        assert_eq!(lc1.structural_hash(), lc2.structural_hash());
    }
}
