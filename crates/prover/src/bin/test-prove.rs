//! Standalone test to verify proof generation with loaded keys

use std::time::Instant;

use ark_bn254::Fr;
use inventory_circuits::Inventory;
use inventory_prover::{prove, setup::CircuitKeys};

fn main() {
    println!("Loading keys from disk...");
    let start = Instant::now();
    let keys = CircuitKeys::load_from_directory(std::path::Path::new("keys"))
        .expect("Failed to load keys");
    println!("Keys loaded in {:?}", start.elapsed());

    println!("\nTesting prove_item_exists...");
    let inventory = Inventory::from_items(&[(1, 100), (2, 50)]);
    let blinding = Fr::from(12345u64);

    let start = Instant::now();
    println!("Starting proof generation...");
    let result = prove::prove_item_exists(&keys.item_exists.proving_key, &inventory, blinding, 1, 50);
    println!("Proof generation completed in {:?}", start.elapsed());

    match result {
        Ok(proof) => {
            println!("Proof generated successfully!");
            println!("Public inputs: {} elements", proof.public_inputs.len());
        }
        Err(e) => {
            eprintln!("Proof generation failed: {}", e);
            std::process::exit(1);
        }
    }
}
