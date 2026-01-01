//! CLI for R1CS Optimizer.
//!
//! This binary provides a command-line interface for the R1CS optimizer.

use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 || args.contains(&"--help".to_string()) {
        print_usage();
        return;
    }

    println!("R1CS Optimizer CLI");
    println!("==================");
    println!();
    println!("This CLI is a placeholder. To optimize circuits, use the library API:");
    println!();
    println!("  use ark_bn254::Fr;");
    println!("  use ark_relations::r1cs::ConstraintSystem;");
    println!("  use r1cs_optimizer::{{Optimizer, OptimizerConfig}};");
    println!();
    println!("  let cs = ConstraintSystem::<Fr>::new_ref();");
    println!("  my_circuit.generate_constraints(cs.clone())?;");
    println!("  cs.finalize();");
    println!();
    println!("  // Optimize");
    println!("  let result = Optimizer::from_cs(cs).optimize();");
    println!();
    println!("  println!(\"Reduced {{}} → {{}} constraints\",");
    println!("      result.original_constraints,");
    println!("      result.final_constraints);");
}

fn print_usage() {
    println!("R1CS Optimizer - Reduce constraint systems");
    println!();
    println!("USAGE:");
    println!("    r1cs-optimize [OPTIONS]");
    println!();
    println!("OPTIONS:");
    println!("    --help        Print this help message");
    println!();
    println!("LIBRARY USAGE:");
    println!("    This tool is primarily used as a library. Add to your Cargo.toml:");
    println!();
    println!("    [dependencies]");
    println!("    r1cs-optimizer = \"0.1\"");
    println!();
    println!("REDUCTION PASSES:");
    println!("    - Deduplication:      Remove identical constraints");
    println!("    - Constant Folding:   Verify and remove constant expressions");
    println!("    - Linear Substitution: Inline simple variable definitions");
    println!("    - Dead Variables:      Remove unused computations");
    println!("    - Common Subexpr:      Report repeated patterns");
}
