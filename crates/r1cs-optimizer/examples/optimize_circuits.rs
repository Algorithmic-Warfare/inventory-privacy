//! Example: Optimize the inventory-privacy circuits
//!
//! Run with: cargo run --example optimize_circuits -p r1cs-optimizer

use ark_bn254::Fr;
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystem};
use r1cs_optimizer::{Optimizer, OptimizerConfig};

use inventory_circuits::{StateTransitionCircuit, ItemExistsSMTCircuit, CapacitySMTCircuit};

fn main() {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║           R1CS OPTIMIZER - INVENTORY CIRCUITS                ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    optimize_circuit("StateTransition", StateTransitionCircuit::empty());
    optimize_circuit("ItemExistsSMT", ItemExistsSMTCircuit::empty());
    optimize_circuit("CapacitySMT", CapacitySMTCircuit::empty());
}

fn optimize_circuit<C: ConstraintSynthesizer<Fr>>(name: &str, circuit: C) {
    println!("───────────────────────────────────────────────────────────────");
    println!("  Circuit: {}", name);
    println!("───────────────────────────────────────────────────────────────");

    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit.generate_constraints(cs.clone()).expect("constraint generation failed");
    cs.finalize();

    let optimizer = Optimizer::from_cs(cs);

    // Show stats before optimization
    let stats = optimizer.stats();
    println!();
    println!("  BEFORE:");
    println!("    Constraints:    {:>6}", stats.num_constraints);
    println!("    Variables:      {:>6}", stats.num_variables);
    println!("    Linear:         {:>6} ({:.1}%)",
        stats.linear_constraints,
        100.0 * stats.linear_constraints as f64 / stats.num_constraints.max(1) as f64);
    println!("    Boolean:        {:>6}", stats.boolean_constraints);
    println!("    Constant:       {:>6}", stats.constant_constraints);
    println!("    Density:        {:>6.4}%", stats.matrix_density * 100.0);

    // Optimize with default config
    let result = optimizer.with_config(OptimizerConfig::default()).optimize();

    println!();
    println!("  AFTER:");
    println!("    Constraints:    {:>6}", result.final_constraints);
    println!("    Reduced:        {:>6} ({:.2}%)",
        result.constraints_reduced(),
        result.reduction_percentage());

    // Show pass reports
    if !result.pass_reports.is_empty() {
        println!();
        println!("  PASSES:");
        for report in &result.pass_reports {
            if report.estimated_savings > 0 {
                println!("    {} - {} patterns, {} savings",
                    report.pass_name,
                    report.patterns_found,
                    report.estimated_savings);
            }
        }
    }

    println!();
}
