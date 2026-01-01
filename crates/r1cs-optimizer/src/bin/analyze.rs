//! CLI for R1CS Analyzer.
//!
//! This binary can analyze constraint systems from various sources.
//!
//! Usage:
//!   analyze --circuit <circuit-name>
//!
//! Currently supports analyzing circuits from the inventory-circuits crate.

use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 || args.contains(&"--help".to_string()) {
        print_usage();
        return;
    }

    println!("R1CS Analyzer CLI");
    println!("=================");
    println!();
    println!("Note: This CLI is a placeholder. To analyze circuits, use the library API:");
    println!();
    println!("  use ark_bn254::Fr;");
    println!("  use ark_relations::r1cs::ConstraintSystem;");
    println!("  use r1cs_analyzer::Analyzer;");
    println!();
    println!("  let cs = ConstraintSystem::<Fr>::new_ref();");
    println!("  // ... generate constraints with your circuit ...");
    println!();
    println!("  let analyzer = Analyzer::from_cs(cs);");
    println!("  let report = analyzer.analyze();");
    println!("  println!(\"{{}}\", report);");
    println!();
    println!("For quick stats without full analysis:");
    println!();
    println!("  let stats = analyzer.quick_stats();");
    println!("  println!(\"{{}}\", stats);");
}

fn print_usage() {
    println!("R1CS Analyzer - Static analysis for constraint systems");
    println!();
    println!("USAGE:");
    println!("    analyze [OPTIONS]");
    println!();
    println!("OPTIONS:");
    println!("    --help        Print this help message");
    println!();
    println!("LIBRARY USAGE:");
    println!("    This tool is primarily used as a library. Add to your Cargo.toml:");
    println!();
    println!("    [dependencies]");
    println!("    r1cs-analyzer = \"0.1\"");
    println!();
    println!("FEATURES:");
    println!("    - Duplicate constraint detection");
    println!("    - Linear constraint identification");
    println!("    - Constant propagation analysis");
    println!("    - Common subexpression detection");
    println!("    - Sparsity pattern analysis");
    println!("    - Variable usage statistics");
}
