//! Export verifying keys for Sui deployment.
//!
//! This tool generates or loads verifying keys and exports them as hex strings
//! suitable for use in Sui Move contracts.

use std::path::Path;

use inventory_prover::setup::{setup_all_circuits, CircuitKeys};

fn main() {
    let keys_dir = Path::new("keys");

    println!("Loading or generating circuit keys...");

    let keys = if keys_dir.exists() {
        println!("Loading existing keys from {:?}", keys_dir);
        CircuitKeys::load_from_directory(keys_dir).expect("Failed to load keys")
    } else {
        println!("Running trusted setup (this may take a while)...");
        let keys = setup_all_circuits().expect("Failed to setup circuits");
        keys.save_to_directory(keys_dir)
            .expect("Failed to save keys");
        println!("Keys saved to {:?}", keys_dir);
        keys
    };

    println!("\n=== Verifying Keys for Sui Deployment ===\n");

    let item_exists_vk = keys.item_exists.serialize_vk().unwrap();
    let withdraw_vk = keys.withdraw.serialize_vk().unwrap();
    let deposit_vk = keys.deposit.serialize_vk().unwrap();
    let transfer_vk = keys.transfer.serialize_vk().unwrap();

    println!("ItemExists VK ({} bytes):", item_exists_vk.len());
    println!("0x{}\n", hex::encode(&item_exists_vk));

    println!("Withdraw VK ({} bytes):", withdraw_vk.len());
    println!("0x{}\n", hex::encode(&withdraw_vk));

    println!("Deposit VK ({} bytes):", deposit_vk.len());
    println!("0x{}\n", hex::encode(&deposit_vk));

    println!("Transfer VK ({} bytes):", transfer_vk.len());
    println!("0x{}\n", hex::encode(&transfer_vk));

    // Also export as JSON for scripting
    let json = serde_json::json!({
        "item_exists_vk": format!("0x{}", hex::encode(&item_exists_vk)),
        "withdraw_vk": format!("0x{}", hex::encode(&withdraw_vk)),
        "deposit_vk": format!("0x{}", hex::encode(&deposit_vk)),
        "transfer_vk": format!("0x{}", hex::encode(&transfer_vk)),
    });

    let json_path = keys_dir.join("verifying_keys.json");
    std::fs::write(&json_path, serde_json::to_string_pretty(&json).unwrap())
        .expect("Failed to write JSON");
    println!("JSON exported to {:?}", json_path);

    // Export as Move vector literals for easy copy-paste
    println!("\n=== Move Vector Literals ===\n");
    println!("// Copy these into your deployment script");
    println!("let item_exists_vk = {};", format_as_move_vector(&item_exists_vk));
    println!("let withdraw_vk = {};", format_as_move_vector(&withdraw_vk));
    println!("let deposit_vk = {};", format_as_move_vector(&deposit_vk));
    println!("let transfer_vk = {};", format_as_move_vector(&transfer_vk));
}

fn format_as_move_vector(bytes: &[u8]) -> String {
    let hex_bytes: Vec<String> = bytes.iter().map(|b| format!("0x{:02x}", b)).collect();
    format!("vector[{}]", hex_bytes.join(", "))
}
