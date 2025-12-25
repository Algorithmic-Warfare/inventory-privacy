//! Integration tests for all circuits.

use std::sync::Arc;

use ark_bn254::{Bn254, Fr};
use ark_groth16::Groth16;
use ark_snark::SNARK;
use ark_std::rand::thread_rng;

use crate::commitment::{create_inventory_commitment, poseidon_config};
use crate::deposit::DepositCircuit;
use crate::inventory::Inventory;
use crate::item_exists::ItemExistsCircuit;
use crate::transfer::TransferCircuit;
use crate::withdraw::WithdrawCircuit;

/// Test full Groth16 proof generation and verification for ItemExistsCircuit
#[test]
fn test_item_exists_full_proof() {
    let mut rng = thread_rng();
    let config = Arc::new(poseidon_config::<Fr>());

    // Setup
    let empty_circuit = ItemExistsCircuit::empty(config.clone());
    let (pk, vk) = Groth16::<Bn254>::circuit_specific_setup(empty_circuit, &mut rng).unwrap();

    // Create inventory and commitment
    let inventory = Inventory::from_items(&[(1, 100), (2, 50)]);
    let blinding = Fr::from(12345u64);
    let commitment = create_inventory_commitment(&inventory, blinding, &config);

    // Create proof circuit
    let circuit = ItemExistsCircuit::new(
        inventory,
        blinding,
        commitment,
        1,  // item_id
        50, // min_quantity
        config,
    );

    // Generate proof
    let proof = Groth16::<Bn254>::prove(&pk, circuit, &mut rng).unwrap();

    // Prepare public inputs: [commitment, item_id, min_quantity]
    let public_inputs = vec![commitment, Fr::from(1u64), Fr::from(50u64)];

    // Verify proof
    let valid = Groth16::<Bn254>::verify(&vk, &public_inputs, &proof).unwrap();
    assert!(valid, "Proof verification failed");
}

/// Test full Groth16 proof for WithdrawCircuit
#[test]
fn test_withdraw_full_proof() {
    let mut rng = thread_rng();
    let config = Arc::new(poseidon_config::<Fr>());

    // Setup
    let empty_circuit = WithdrawCircuit::empty(config.clone());
    let (pk, vk) = Groth16::<Bn254>::circuit_specific_setup(empty_circuit, &mut rng).unwrap();

    // Create inventories
    let old_inventory = Inventory::from_items(&[(1, 100)]);
    let old_blinding = Fr::from(12345u64);
    let old_commitment = create_inventory_commitment(&old_inventory, old_blinding, &config);

    let mut new_inventory = old_inventory.clone();
    new_inventory.withdraw(1, 30).unwrap();
    let new_blinding = Fr::from(67890u64);
    let new_commitment = create_inventory_commitment(&new_inventory, new_blinding, &config);

    // Create proof circuit
    let circuit = WithdrawCircuit::new(
        old_inventory,
        new_inventory,
        old_blinding,
        new_blinding,
        old_commitment,
        new_commitment,
        1,  // item_id
        30, // amount
        config,
    );

    // Generate proof
    let proof = Groth16::<Bn254>::prove(&pk, circuit, &mut rng).unwrap();

    // Prepare public inputs
    let public_inputs = vec![
        old_commitment,
        new_commitment,
        Fr::from(1u64),  // item_id
        Fr::from(30u64), // amount
    ];

    // Verify proof
    let valid = Groth16::<Bn254>::verify(&vk, &public_inputs, &proof).unwrap();
    assert!(valid, "Withdraw proof verification failed");
}

/// Test full Groth16 proof for DepositCircuit
#[test]
fn test_deposit_full_proof() {
    let mut rng = thread_rng();
    let config = Arc::new(poseidon_config::<Fr>());

    // Setup
    let empty_circuit = DepositCircuit::empty(config.clone());
    let (pk, vk) = Groth16::<Bn254>::circuit_specific_setup(empty_circuit, &mut rng).unwrap();

    // Create inventories
    let old_inventory = Inventory::from_items(&[(1, 50)]);
    let old_blinding = Fr::from(12345u64);
    let old_commitment = create_inventory_commitment(&old_inventory, old_blinding, &config);

    let mut new_inventory = old_inventory.clone();
    new_inventory.deposit(1, 25).unwrap();
    let new_blinding = Fr::from(67890u64);
    let new_commitment = create_inventory_commitment(&new_inventory, new_blinding, &config);

    // Create proof circuit
    let circuit = DepositCircuit::new(
        old_inventory,
        new_inventory,
        old_blinding,
        new_blinding,
        old_commitment,
        new_commitment,
        1,  // item_id
        25, // amount
        config,
    );

    // Generate proof
    let proof = Groth16::<Bn254>::prove(&pk, circuit, &mut rng).unwrap();

    // Prepare public inputs
    let public_inputs = vec![
        old_commitment,
        new_commitment,
        Fr::from(1u64),  // item_id
        Fr::from(25u64), // amount
    ];

    // Verify proof
    let valid = Groth16::<Bn254>::verify(&vk, &public_inputs, &proof).unwrap();
    assert!(valid, "Deposit proof verification failed");
}

/// Test full Groth16 proof for TransferCircuit
#[test]
fn test_transfer_full_proof() {
    let mut rng = thread_rng();
    let config = Arc::new(poseidon_config::<Fr>());

    // Setup
    let empty_circuit = TransferCircuit::empty(config.clone());
    let (pk, vk) = Groth16::<Bn254>::circuit_specific_setup(empty_circuit, &mut rng).unwrap();

    // Source inventory
    let src_old = Inventory::from_items(&[(1, 100)]);
    let src_old_blinding = Fr::from(111u64);
    let src_old_commitment = create_inventory_commitment(&src_old, src_old_blinding, &config);

    let mut src_new = src_old.clone();
    src_new.withdraw(1, 40).unwrap();
    let src_new_blinding = Fr::from(222u64);
    let src_new_commitment = create_inventory_commitment(&src_new, src_new_blinding, &config);

    // Destination inventory
    let dst_old = Inventory::from_items(&[(1, 10)]);
    let dst_old_blinding = Fr::from(333u64);
    let dst_old_commitment = create_inventory_commitment(&dst_old, dst_old_blinding, &config);

    let mut dst_new = dst_old.clone();
    dst_new.deposit(1, 40).unwrap();
    let dst_new_blinding = Fr::from(444u64);
    let dst_new_commitment = create_inventory_commitment(&dst_new, dst_new_blinding, &config);

    // Create proof circuit
    let circuit = TransferCircuit::new(
        src_old,
        src_new,
        src_old_blinding,
        src_new_blinding,
        dst_old,
        dst_new,
        dst_old_blinding,
        dst_new_blinding,
        src_old_commitment,
        src_new_commitment,
        dst_old_commitment,
        dst_new_commitment,
        1,  // item_id
        40, // amount
        config,
    );

    // Generate proof
    let proof = Groth16::<Bn254>::prove(&pk, circuit, &mut rng).unwrap();

    // Prepare public inputs
    let public_inputs = vec![
        src_old_commitment,
        src_new_commitment,
        dst_old_commitment,
        dst_new_commitment,
        Fr::from(1u64),  // item_id
        Fr::from(40u64), // amount
    ];

    // Verify proof
    let valid = Groth16::<Bn254>::verify(&vk, &public_inputs, &proof).unwrap();
    assert!(valid, "Transfer proof verification failed");
}

/// Test that invalid proofs are rejected
#[test]
fn test_invalid_proof_rejected() {
    let mut rng = thread_rng();
    let config = Arc::new(poseidon_config::<Fr>());

    // Setup
    let empty_circuit = ItemExistsCircuit::empty(config.clone());
    let (pk, vk) = Groth16::<Bn254>::circuit_specific_setup(empty_circuit, &mut rng).unwrap();

    // Create valid proof
    let inventory = Inventory::from_items(&[(1, 100)]);
    let blinding = Fr::from(12345u64);
    let commitment = create_inventory_commitment(&inventory, blinding, &config);

    let circuit = ItemExistsCircuit::new(inventory, blinding, commitment, 1, 50, config);

    let proof = Groth16::<Bn254>::prove(&pk, circuit, &mut rng).unwrap();

    // Try to verify with WRONG public inputs (different min_quantity)
    let wrong_public_inputs = vec![
        commitment,
        Fr::from(1u64),
        Fr::from(200u64), // Wrong! We proved >= 50, not >= 200
    ];

    let valid = Groth16::<Bn254>::verify(&vk, &wrong_public_inputs, &proof).unwrap();
    assert!(!valid, "Invalid proof should be rejected");
}
