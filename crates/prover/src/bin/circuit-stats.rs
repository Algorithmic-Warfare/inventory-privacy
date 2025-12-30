//! Circuit statistics utility - reports constraint counts and proof timing
//!
//! Usage:
//!   cargo run --release --bin circuit-stats           # Just constraint counts
//!   cargo run --release --bin circuit-stats -- --time # Include proof timing (needs keys)

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use ark_bn254::Fr;
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystem};

use inventory_circuits::{
    poseidon_config,
    CapacitySMTCircuit,
    ItemExistsSMTCircuit,
    StateTransitionCircuit,
    DEFAULT_DEPTH,
    OpType,
};

fn count_constraints<C: ConstraintSynthesizer<Fr>>(circuit: C, name: &str) -> usize {
    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit.generate_constraints(cs.clone()).unwrap();
    let count = cs.num_constraints();
    // Note: empty circuits use dummy values so they may not satisfy all constraints
    // The constraint count is still accurate
    println!("{:25} {:>8} constraints", name, count);
    count
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let include_timing = args.iter().any(|a| a == "--time");

    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║            INVENTORY PRIVACY CIRCUIT STATS               ║");
    println!("╚══════════════════════════════════════════════════════════╝\n");

    // Build poseidon config
    let config = Arc::new(poseidon_config::<Fr>());

    println!("SMT Depth: {}", DEFAULT_DEPTH);
    println!("Max Items: {}\n", 1u64 << DEFAULT_DEPTH);

    println!("─────────────────────────────────────────────────────────────");
    println!("CIRCUIT CONSTRAINTS:");
    println!("─────────────────────────────────────────────────────────────\n");

    // StateTransition - use empty constructor
    let state_circuit = StateTransitionCircuit::<Fr>::empty(config.clone());
    let state_count = count_constraints(state_circuit, "StateTransition");

    // ItemExists - use empty constructor
    let item_circuit = ItemExistsSMTCircuit::<Fr>::empty(config.clone());
    let item_count = count_constraints(item_circuit, "ItemExists");

    // Capacity - use empty constructor
    let cap_circuit = CapacitySMTCircuit::<Fr>::empty(config.clone());
    let cap_count = count_constraints(cap_circuit, "CapacityProof");

    println!("\n─────────────────────────────────────────────────────────────");
    println!("SUMMARY:");
    println!("─────────────────────────────────────────────────────────────\n");

    println!("Total constraints (all circuits): {}", state_count + item_count + cap_count);
    println!();

    // Update Home page values comparison
    println!("Circuit          Actual    Plan Est.    Delta");
    println!("──────────────────────────────────────────────");
    println!("StateTransition   {:>6}       ~8,500   {:+}", state_count, state_count as i32 - 8500);
    println!("ItemExists        {:>6}       ~4,200   {:+}", item_count, item_count as i32 - 4200);
    println!("CapacityProof     {:>6}         ~900   {:+}", cap_count, cap_count as i32 - 900);

    if include_timing {
        println!("\n─────────────────────────────────────────────────────────────");
        println!("PROOF TIMING:");
        println!("─────────────────────────────────────────────────────────────\n");

        let keys_path = Path::new("keys");
        if !keys_path.exists() {
            // Try common locations
            let alt_paths = [
                Path::new("../../keys"),
                Path::new("../keys"),
                Path::new("./target/keys"),
            ];

            let found_path = alt_paths.iter().find(|p| p.exists());

            if let Some(path) = found_path {
                run_timing_benchmarks(path);
            } else {
                println!("Keys not found. Generate keys first with:");
                println!("  cargo run --release -p inventory-prover");
                println!("\nOr specify keys location in the code.");
            }
        } else {
            run_timing_benchmarks(keys_path);
        }
    } else {
        println!("\n(Run with --time to include proof generation timing)");
    }
}

fn run_timing_benchmarks(keys_path: &Path) {
    use inventory_prover::{prove, setup::CircuitKeys, InventoryState};

    println!("Loading keys from {:?}...", keys_path);
    let start = Instant::now();
    let keys = match CircuitKeys::load_from_directory(keys_path) {
        Ok(k) => k,
        Err(e) => {
            println!("Failed to load keys: {}", e);
            return;
        }
    };
    println!("Keys loaded in {:?}\n", start.elapsed());

    let blinding = Fr::from(12345u64);
    let mut state = InventoryState::new(blinding);
    state.tree.update(42, 100);
    state.current_volume = 500;

    // Warm up (first proof is slower due to caching)
    let _ = prove::prove_capacity(&keys.capacity.proving_key, &state, 1000);

    // Benchmark each circuit (3 runs each)
    const RUNS: usize = 3;

    // Constraint counts for efficiency calculation
    const CAP_CONSTRAINTS: u128 = 724;
    const ITEM_CONSTRAINTS: u128 = 4124;
    const STATE_CONSTRAINTS: u128 = 8255;

    println!("Circuit          Constraints    Avg Time    μs/constraint");
    println!("────────────────────────────────────────────────────────────");

    // CapacityProof
    let mut times = Vec::new();
    for _ in 0..RUNS {
        let start = Instant::now();
        let _ = prove::prove_capacity(&keys.capacity.proving_key, &state, 1000);
        times.push(start.elapsed().as_micros());
    }
    let avg_us = times.iter().sum::<u128>() / RUNS as u128;
    let us_per_constraint = avg_us as f64 / CAP_CONSTRAINTS as f64;
    println!(
        "CapacityProof         {:>5}       {:>4}ms         {:.2}",
        CAP_CONSTRAINTS,
        avg_us / 1000,
        us_per_constraint
    );

    // ItemExists
    times.clear();
    for _ in 0..RUNS {
        let start = Instant::now();
        let _ = prove::prove_item_exists(&keys.item_exists.proving_key, &state, 42, 50);
        times.push(start.elapsed().as_micros());
    }
    let avg_us = times.iter().sum::<u128>() / RUNS as u128;
    let us_per_constraint = avg_us as f64 / ITEM_CONSTRAINTS as f64;
    println!(
        "ItemExists            {:>5}       {:>4}ms         {:.2}",
        ITEM_CONSTRAINTS,
        avg_us / 1000,
        us_per_constraint
    );

    // StateTransition
    times.clear();
    for _ in 0..RUNS {
        let start = Instant::now();
        let _ = prove::prove_state_transition(
            &keys.state_transition.proving_key,
            &state,
            Fr::from(99999u64), // new_blinding
            42,                 // item_id
            50,                 // amount
            1,                  // item_volume
            Fr::from(0u64),     // registry_root
            1000,               // max_capacity
            OpType::Deposit,    // op_type
        );
        times.push(start.elapsed().as_micros());
    }
    let avg_us = times.iter().sum::<u128>() / RUNS as u128;
    let us_per_constraint = avg_us as f64 / STATE_CONSTRAINTS as f64;
    println!(
        "StateTransition       {:>5}       {:>4}ms         {:.2}",
        STATE_CONSTRAINTS,
        avg_us / 1000,
        us_per_constraint
    );

    // Summary
    let total_constraints = CAP_CONSTRAINTS + ITEM_CONSTRAINTS + STATE_CONSTRAINTS;
    println!("\n────────────────────────────────────────────────────────────");
    println!("Machine Performance Summary:");
    println!("  Total constraints:  {}", total_constraints);
    println!("  Avg μs/constraint:  ~{:.1}", us_per_constraint); // Use last (largest circuit) as reference
}
