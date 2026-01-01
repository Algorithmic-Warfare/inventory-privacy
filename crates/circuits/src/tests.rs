//! Integration tests for SMT-based circuits.

use ark_bn254::{Bn254, Fr};
use ark_groth16::Groth16;
use ark_snark::SNARK;
use ark_std::rand::thread_rng;

use crate::signal::OpType;
use crate::smt::{SparseMerkleTree, DEFAULT_DEPTH};
use crate::state_transition::StateTransitionCircuit;
use crate::item_exists_smt::ItemExistsSMTCircuit;
use crate::capacity_smt::CapacitySMTCircuit;

/// Test full Groth16 proof generation and verification for StateTransitionCircuit (deposit)
#[test]
fn test_state_transition_deposit_full_proof() {
    let mut rng = thread_rng();

    // Setup
    let empty_circuit = StateTransitionCircuit::empty();
    let (pk, vk) = Groth16::<Bn254>::circuit_specific_setup(empty_circuit, &mut rng).unwrap();

    // Create initial inventory
    let mut tree = SparseMerkleTree::from_items(
        &[(1, 100)],
        DEFAULT_DEPTH,
    );
    let old_root = tree.root();
    let proof = tree.get_proof(1);

    // Deposit 50 more
    tree.update(1, 150);
    let new_root = tree.root();

    let old_blinding = Fr::from(12345u64);
    let new_blinding = Fr::from(67890u64);
    let item_volume = 10u64;
    let old_volume = 100 * item_volume;
    let new_volume = 150 * item_volume;
    let registry_root = Fr::from(99999u64);
    let max_capacity = 10000u64;
    let nonce = 0u64;
    let inventory_id = Fr::from(12345678u64);

    // Create proof circuit
    let circuit = StateTransitionCircuit::new(
        old_root,
        old_volume,
        old_blinding,
        new_root,
        new_volume,
        new_blinding,
        1,   // item_id
        100, // old_quantity
        150, // new_quantity
        50,  // amount
        OpType::Deposit,
        proof,
        item_volume,
        registry_root,
        max_capacity,
        nonce,
        inventory_id,
    );

    let signal_hash = circuit.signal_hash.unwrap();

    // Generate proof
    let groth_proof = Groth16::<Bn254>::prove(&pk, circuit, &mut rng).unwrap();

    // Verify proof with all 4 public inputs: signal_hash, nonce, inventory_id, registry_root
    let public_inputs = vec![signal_hash, Fr::from(nonce), inventory_id, registry_root];
    let valid = Groth16::<Bn254>::verify(&vk, &public_inputs, &groth_proof).unwrap();
    assert!(valid, "StateTransition deposit proof verification failed");
}

/// Test full Groth16 proof for StateTransitionCircuit (withdraw)
#[test]
fn test_state_transition_withdraw_full_proof() {
    let mut rng = thread_rng();

    // Setup
    let empty_circuit = StateTransitionCircuit::empty();
    let (pk, vk) = Groth16::<Bn254>::circuit_specific_setup(empty_circuit, &mut rng).unwrap();

    // Create initial inventory
    let mut tree = SparseMerkleTree::from_items(
        &[(1, 100)],
        DEFAULT_DEPTH,
    );
    let old_root = tree.root();
    let proof = tree.get_proof(1);

    // Withdraw 30
    tree.update(1, 70);
    let new_root = tree.root();

    let old_blinding = Fr::from(12345u64);
    let new_blinding = Fr::from(67890u64);
    let item_volume = 10u64;
    let old_volume = 100 * item_volume;
    let new_volume = 70 * item_volume;
    let registry_root = Fr::from(99999u64);
    let max_capacity = 10000u64;
    let nonce = 5u64;
    let inventory_id = Fr::from(12345678u64);

    let circuit = StateTransitionCircuit::new(
        old_root,
        old_volume,
        old_blinding,
        new_root,
        new_volume,
        new_blinding,
        1,
        100,
        70,
        30,
        OpType::Withdraw,
        proof,
        item_volume,
        registry_root,
        max_capacity,
        nonce,
        inventory_id,
    );

    let signal_hash = circuit.signal_hash.unwrap();

    let groth_proof = Groth16::<Bn254>::prove(&pk, circuit, &mut rng).unwrap();

    // Verify proof with all 4 public inputs: signal_hash, nonce, inventory_id, registry_root
    let public_inputs = vec![signal_hash, Fr::from(nonce), inventory_id, registry_root];
    let valid = Groth16::<Bn254>::verify(&vk, &public_inputs, &groth_proof).unwrap();
    assert!(valid, "StateTransition withdraw proof verification failed");
}

/// Test full Groth16 proof for ItemExistsSMTCircuit
#[test]
fn test_item_exists_smt_full_proof() {
    let mut rng = thread_rng();

    // Setup
    let empty_circuit = ItemExistsSMTCircuit::empty();
    let (pk, vk) = Groth16::<Bn254>::circuit_specific_setup(empty_circuit, &mut rng).unwrap();

    // Create inventory
    let tree = SparseMerkleTree::from_items(
        &[(42, 100)],
        DEFAULT_DEPTH,
    );
    let root = tree.root();
    let proof = tree.get_proof(42);

    let blinding = Fr::from(12345u64);
    let volume = 1000u64;

    // Prove we have at least 50 of item 42
    let circuit = ItemExistsSMTCircuit::new(
        root,
        volume,
        blinding,
        42,  // item_id
        100, // actual_quantity
        50,  // min_quantity
        proof,
    );

    let public_hash = circuit.public_hash.unwrap();

    let groth_proof = Groth16::<Bn254>::prove(&pk, circuit, &mut rng).unwrap();

    let public_inputs = vec![public_hash];
    let valid = Groth16::<Bn254>::verify(&vk, &public_inputs, &groth_proof).unwrap();
    assert!(valid, "ItemExists SMT proof verification failed");
}

/// Test full Groth16 proof for CapacitySMTCircuit
#[test]
fn test_capacity_smt_full_proof() {
    let mut rng = thread_rng();

    // Setup
    let empty_circuit = CapacitySMTCircuit::empty();
    let (pk, vk) = Groth16::<Bn254>::circuit_specific_setup(empty_circuit, &mut rng).unwrap();

    // Create inventory
    let tree = SparseMerkleTree::from_items(
        &[(1, 100), (2, 50)],
        DEFAULT_DEPTH,
    );
    let root = tree.root();

    let blinding = Fr::from(12345u64);
    let volume = 500u64;
    let max_capacity = 1000u64;

    let circuit = CapacitySMTCircuit::new(
        root,
        volume,
        blinding,
        max_capacity,
    );

    let public_hash = circuit.public_hash.unwrap();

    let groth_proof = Groth16::<Bn254>::prove(&pk, circuit, &mut rng).unwrap();

    let public_inputs = vec![public_hash];
    let valid = Groth16::<Bn254>::verify(&vk, &public_inputs, &groth_proof).unwrap();
    assert!(valid, "Capacity SMT proof verification failed");
}

/// Test that invalid proofs are rejected
#[test]
fn test_invalid_proof_rejected() {
    let mut rng = thread_rng();

    // Setup
    let empty_circuit = ItemExistsSMTCircuit::empty();
    let (pk, vk) = Groth16::<Bn254>::circuit_specific_setup(empty_circuit, &mut rng).unwrap();

    // Create valid proof
    let tree = SparseMerkleTree::from_items(
        &[(1, 100)],
        DEFAULT_DEPTH,
    );
    let root = tree.root();
    let proof = tree.get_proof(1);

    let blinding = Fr::from(12345u64);
    let volume = 1000u64;

    let circuit = ItemExistsSMTCircuit::new(
        root,
        volume,
        blinding,
        1,
        100,
        50,
        proof,
    );

    let groth_proof = Groth16::<Bn254>::prove(&pk, circuit, &mut rng).unwrap();

    // Try to verify with WRONG public hash
    let wrong_public_inputs = vec![Fr::from(99999u64)];

    let valid = Groth16::<Bn254>::verify(&vk, &wrong_public_inputs, &groth_proof).unwrap();
    assert!(!valid, "Invalid proof should be rejected");
}
