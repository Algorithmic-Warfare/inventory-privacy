//! R1CS Optimizer - orchestrates reduction passes.
//!
//! The optimizer runs a configurable sequence of reduction passes over
//! a constraint matrix, producing an optimized matrix with fewer constraints.

use ark_ff::PrimeField;
use ark_relations::r1cs::ConstraintSystemRef;

use crate::constraint::ConstraintMatrix;
use crate::passes::{
    CommonSubexpressionPass, ConstantFoldingPass, DeadVariablePass, DeduplicationPass,
    LinearSubstitutionPass, ReductionPass,
};
use crate::reduction::{OptimizationResult, ReductionReport};

/// Configuration for the optimizer.
#[derive(Clone, Debug)]
pub struct OptimizerConfig {
    /// Run deduplication pass
    pub deduplicate: bool,
    /// Run constant folding pass
    pub fold_constants: bool,
    /// Run linear substitution pass
    pub substitute_linear: bool,
    /// Run dead variable elimination pass
    pub eliminate_dead: bool,
    /// Run common subexpression detection (informational only)
    pub detect_cse: bool,
    /// Maximum iterations for fixed-point optimization
    pub max_iterations: usize,
}

impl Default for OptimizerConfig {
    fn default() -> Self {
        Self {
            deduplicate: true,
            fold_constants: true,
            substitute_linear: true,
            eliminate_dead: true,
            detect_cse: true,
            max_iterations: 10,
        }
    }
}

impl OptimizerConfig {
    /// Create a config that only runs safe, guaranteed reductions.
    pub fn safe() -> Self {
        Self {
            deduplicate: true,
            fold_constants: true,
            substitute_linear: false, // Can change constraint structure
            eliminate_dead: false,    // Requires careful analysis
            detect_cse: true,
            max_iterations: 3,
        }
    }

    /// Create a config that runs all passes aggressively.
    pub fn aggressive() -> Self {
        Self {
            deduplicate: true,
            fold_constants: true,
            substitute_linear: true,
            eliminate_dead: true,
            detect_cse: true,
            max_iterations: 20,
        }
    }

    /// Create a config for analysis only (no transformations).
    pub fn analyze_only() -> Self {
        Self {
            deduplicate: false,
            fold_constants: false,
            substitute_linear: false,
            eliminate_dead: false,
            detect_cse: true,
            max_iterations: 1,
        }
    }
}

/// The R1CS Optimizer.
///
/// Orchestrates running reduction passes over a constraint matrix.
pub struct Optimizer<F: PrimeField> {
    matrix: ConstraintMatrix<F>,
    config: OptimizerConfig,
}

impl<F: PrimeField> Optimizer<F> {
    /// Create an optimizer from an arkworks constraint system.
    pub fn from_cs(cs: ConstraintSystemRef<F>) -> Self {
        Self {
            matrix: ConstraintMatrix::from_cs(cs),
            config: OptimizerConfig::default(),
        }
    }

    /// Create an optimizer from a constraint matrix.
    pub fn from_matrix(matrix: ConstraintMatrix<F>) -> Self {
        Self {
            matrix,
            config: OptimizerConfig::default(),
        }
    }

    /// Set the optimizer configuration.
    pub fn with_config(mut self, config: OptimizerConfig) -> Self {
        self.config = config;
        self
    }

    /// Get a reference to the current matrix.
    pub fn matrix(&self) -> &ConstraintMatrix<F> {
        &self.matrix
    }

    /// Run all configured passes and return the optimization result.
    pub fn optimize(self) -> OptimizationResult<F> {
        let original_constraints = self.matrix.num_constraints();
        let mut matrix = self.matrix;
        let mut all_reports = Vec::new();

        // Run passes iteratively until no more reductions
        for iteration in 0..self.config.max_iterations {
            let before = matrix.num_constraints();
            let mut iteration_reports = Vec::new();

            // Deduplication
            if self.config.deduplicate {
                let pass = DeduplicationPass::new();
                let (new_matrix, report) = pass.optimize(matrix);
                if report.estimated_savings > 0 {
                    iteration_reports.push(report);
                }
                matrix = new_matrix;
            }

            // Constant folding
            if self.config.fold_constants {
                let pass = ConstantFoldingPass::new();
                let (new_matrix, report) = pass.optimize(matrix);
                if report.estimated_savings > 0 {
                    iteration_reports.push(report);
                }
                matrix = new_matrix;
            }

            // Linear substitution
            if self.config.substitute_linear {
                let pass = LinearSubstitutionPass::new();
                let (new_matrix, report) = pass.optimize(matrix);
                if report.estimated_savings > 0 {
                    iteration_reports.push(report);
                }
                matrix = new_matrix;
            }

            // Dead variable elimination
            if self.config.eliminate_dead {
                let pass = DeadVariablePass::new();
                let (new_matrix, report) = pass.optimize(matrix);
                if report.estimated_savings > 0 {
                    iteration_reports.push(report);
                }
                matrix = new_matrix;
            }

            all_reports.extend(iteration_reports);

            // Check for fixed point
            let after = matrix.num_constraints();
            if after >= before {
                break; // No progress, stop iterating
            }

            if iteration > 0 {
                // Add iteration marker
                let mut iter_report = ReductionReport::new(format!("Iteration {}", iteration + 1));
                iter_report.estimated_savings = before - after;
                iter_report.add_finding(format!("Reduced {} → {} constraints", before, after));
                all_reports.push(iter_report);
            }
        }

        // Run CSE detection (informational only)
        if self.config.detect_cse {
            let pass = CommonSubexpressionPass::new();
            let matches = pass.scan(&matrix);
            if !matches.is_empty() {
                all_reports.push(<CommonSubexpressionPass as ReductionPass<F>>::report(&pass, &matches));
            }
        }

        let final_constraints = matrix.num_constraints();
        OptimizationResult {
            matrix,
            original_constraints,
            final_constraints,
            pass_reports: all_reports,
        }
    }

    /// Analyze without modifying - returns reports only.
    pub fn analyze(&self) -> Vec<ReductionReport> {
        let mut reports = Vec::new();

        // Run all passes in scan-only mode
        let dedup = DeduplicationPass::new();
        let dedup_matches = dedup.scan(&self.matrix);
        if !dedup_matches.is_empty() {
            reports.push(<DeduplicationPass as ReductionPass<F>>::report(&dedup, &dedup_matches));
        }

        let const_fold = ConstantFoldingPass::new();
        let const_matches = const_fold.scan(&self.matrix);
        if !const_matches.is_empty() {
            reports.push(<ConstantFoldingPass as ReductionPass<F>>::report(&const_fold, &const_matches));
        }

        let linear = LinearSubstitutionPass::new();
        let linear_matches = linear.scan(&self.matrix);
        if !linear_matches.is_empty() {
            reports.push(<LinearSubstitutionPass as ReductionPass<F>>::report(&linear, &linear_matches));
        }

        let dead = DeadVariablePass::new();
        let dead_matches = dead.scan(&self.matrix);
        if !dead_matches.is_empty() {
            reports.push(<DeadVariablePass as ReductionPass<F>>::report(&dead, &dead_matches));
        }

        let cse = CommonSubexpressionPass::new();
        let cse_matches = cse.scan(&self.matrix);
        if !cse_matches.is_empty() {
            reports.push(<CommonSubexpressionPass as ReductionPass<F>>::report(&cse, &cse_matches));
        }

        reports
    }

    /// Get quick statistics about the constraint matrix.
    pub fn stats(&self) -> MatrixStats {
        let sparsity = self.matrix.sparsity_stats();

        let mut linear_count = 0;
        let mut boolean_count = 0;
        let mut constant_count = 0;

        for constraint in &self.matrix.constraints {
            if constraint.is_linear() {
                linear_count += 1;
            }
            if constraint.is_boolean() {
                boolean_count += 1;
            }
            if constraint.is_constant() {
                constant_count += 1;
            }
        }

        MatrixStats {
            num_constraints: self.matrix.num_constraints(),
            num_variables: self.matrix.num_variables,
            num_public_inputs: self.matrix.num_public_inputs,
            num_private_witnesses: self.matrix.num_private_witnesses,
            linear_constraints: linear_count,
            boolean_constraints: boolean_count,
            constant_constraints: constant_count,
            matrix_density: sparsity.density,
            avg_terms_per_constraint: sparsity.avg_terms_per_constraint,
        }
    }
}

/// Statistics about a constraint matrix.
#[derive(Clone, Debug)]
pub struct MatrixStats {
    pub num_constraints: usize,
    pub num_variables: usize,
    pub num_public_inputs: usize,
    pub num_private_witnesses: usize,
    pub linear_constraints: usize,
    pub boolean_constraints: usize,
    pub constant_constraints: usize,
    pub matrix_density: f64,
    pub avg_terms_per_constraint: f64,
}

impl std::fmt::Display for MatrixStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "R1CS Matrix Statistics:")?;
        writeln!(f, "  Constraints:        {:>8}", self.num_constraints)?;
        writeln!(f, "  Variables:          {:>8}", self.num_variables)?;
        writeln!(f, "    - Public inputs:  {:>8}", self.num_public_inputs)?;
        writeln!(f, "    - Private:        {:>8}", self.num_private_witnesses)?;
        writeln!(f, "  Linear:             {:>8} ({:.1}%)",
            self.linear_constraints,
            100.0 * self.linear_constraints as f64 / self.num_constraints.max(1) as f64)?;
        writeln!(f, "  Boolean:            {:>8}", self.boolean_constraints)?;
        writeln!(f, "  Constant:           {:>8}", self.constant_constraints)?;
        writeln!(f, "  Matrix density:     {:>8.4}%", self.matrix_density * 100.0)?;
        writeln!(f, "  Avg terms/constr:   {:>8.2}", self.avg_terms_per_constraint)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_bn254::Fr;
    use ark_relations::r1cs::{
        ConstraintSynthesizer, ConstraintSystem, ConstraintSystemRef,
        LinearCombination as ArkLC, SynthesisError, Variable,
    };

    struct TestCircuit;

    impl ConstraintSynthesizer<Fr> for TestCircuit {
        fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
            let x = cs.new_witness_variable(|| Ok(Fr::from(3u64)))?;
            let y = cs.new_witness_variable(|| Ok(Fr::from(5u64)))?;
            let z = cs.new_witness_variable(|| Ok(Fr::from(15u64)))?;

            // Regular constraint
            cs.enforce_constraint(ArkLC::from(x), ArkLC::from(y), ArkLC::from(z))?;

            // Duplicate
            cs.enforce_constraint(ArkLC::from(x), ArkLC::from(y), ArkLC::from(z))?;

            // Constant constraint
            cs.enforce_constraint(
                ArkLC::zero() + (Fr::from(2u64), Variable::One),
                ArkLC::zero() + (Fr::from(3u64), Variable::One),
                ArkLC::zero() + (Fr::from(6u64), Variable::One),
            )?;

            Ok(())
        }
    }

    #[test]
    fn test_optimizer_default() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        TestCircuit.generate_constraints(cs.clone()).unwrap();
        cs.finalize();

        let optimizer = Optimizer::from_cs(cs);
        let result = optimizer.optimize();

        assert!(
            result.final_constraints < result.original_constraints,
            "Should reduce constraints"
        );
    }

    #[test]
    fn test_optimizer_analyze() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        TestCircuit.generate_constraints(cs.clone()).unwrap();
        cs.finalize();

        let optimizer = Optimizer::from_cs(cs);
        let reports = optimizer.analyze();

        assert!(!reports.is_empty(), "Should produce reports");
    }

    #[test]
    fn test_optimizer_stats() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        TestCircuit.generate_constraints(cs.clone()).unwrap();
        cs.finalize();

        let optimizer = Optimizer::from_cs(cs);
        let stats = optimizer.stats();

        assert_eq!(stats.num_constraints, 3);
    }
}
