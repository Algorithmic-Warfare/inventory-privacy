//! Integration tests for the SMT module.

use super::*;
use ark_bn254::Fr;
use ark_relations::r1cs::ConstraintSystem;
use ark_r1cs_std::prelude::*;
use ark_r1cs_std::fields::fp::FpVar;

#[test]
fn test_full_workflow() {
    // Create tree with some items
    let items = vec![
        (1, 100),   // Sword: 100
        (5, 50),    // Potion: 50
        (100, 25),  // Rare item: 25
    ];

    let tree = SparseMerkleTree::from_items(&items, DEFAULT_DEPTH);

    // Verify all items
    for &(item_id, quantity) in &items {
        let proof = tree.get_proof(item_id);
        assert!(tree.verify_proof(item_id, quantity, &proof));
    }

    // Verify empty slots return 0
    assert_eq!(tree.get(999), 0);
}

#[test]
fn test_deposit_workflow() {
    let mut tree = SparseMerkleTree::new(DEFAULT_DEPTH);

    // Initial state: empty
    let initial_root = tree.root();

    // Deposit 100 gold
    tree.update(1, 100);
    let after_deposit = tree.root();

    assert_ne!(initial_root, after_deposit);
    assert_eq!(tree.get(1), 100);

    // Generate proof for verification
    let proof = tree.get_proof(1);
    assert!(tree.verify_proof(1, 100, &proof));
}

#[test]
fn test_withdraw_workflow() {
    let mut tree = SparseMerkleTree::from_items(&[(1, 100)], DEFAULT_DEPTH);

    // Get proof before withdraw
    let old_root = tree.root();
    let proof = tree.get_proof(1);

    // Withdraw 30 (new quantity = 70)
    tree.update(1, 70);
    let new_root = tree.root();

    assert_ne!(old_root, new_root);
    assert_eq!(tree.get(1), 70);

    // Old proof should still be valid for OLD state
    // (This is how we verify the old state in circuits)
    let old_computed = proof.compute_root(1, 100);
    assert_eq!(old_computed, old_root);
}

#[test]
fn test_circuit_deposit() {
    // Initial inventory with 1 item
    let mut tree = SparseMerkleTree::from_items(&[(1, 100)], DEFAULT_DEPTH);

    let old_root = tree.root();
    let proof = tree.get_proof(1);

    // Deposit 50 more
    tree.update(1, 150);
    let new_root = tree.root();

    // Verify in circuit
    let cs = ConstraintSystem::<Fr>::new_ref();

    let old_root_var = FpVar::new_input(cs.clone(), || Ok(old_root)).unwrap();
    let new_root_var = FpVar::new_input(cs.clone(), || Ok(new_root)).unwrap();
    let item_id_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(1u64))).unwrap();
    let old_qty_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(100u64))).unwrap();
    let new_qty_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(150u64))).unwrap();
    let proof_var = MerkleProofVar::new_witness(cs.clone(), &proof).unwrap();

    // Verify old state and compute new root
    let computed_new_root = verify_and_update(
        cs.clone(),
        &old_root_var,
        &item_id_var,
        &old_qty_var,
        &new_qty_var,
        &proof_var,
    ).unwrap();

    // Enforce new root matches
    computed_new_root.enforce_equal(&new_root_var).unwrap();

    assert!(cs.is_satisfied().unwrap());
    println!("Deposit circuit satisfied with {} constraints", cs.num_constraints());
}

#[test]
fn test_circuit_withdraw() {
    let mut tree = SparseMerkleTree::from_items(&[(1, 100)], DEFAULT_DEPTH);

    let old_root = tree.root();
    let proof = tree.get_proof(1);

    // Withdraw 30
    tree.update(1, 70);
    let new_root = tree.root();

    let cs = ConstraintSystem::<Fr>::new_ref();

    let old_root_var = FpVar::new_input(cs.clone(), || Ok(old_root)).unwrap();
    let new_root_var = FpVar::new_input(cs.clone(), || Ok(new_root)).unwrap();
    let item_id_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(1u64))).unwrap();
    let old_qty_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(100u64))).unwrap();
    let new_qty_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(70u64))).unwrap();
    let proof_var = MerkleProofVar::new_witness(cs.clone(), &proof).unwrap();

    let computed_new_root = verify_and_update(
        cs.clone(),
        &old_root_var,
        &item_id_var,
        &old_qty_var,
        &new_qty_var,
        &proof_var,
    ).unwrap();

    computed_new_root.enforce_equal(&new_root_var).unwrap();

    assert!(cs.is_satisfied().unwrap());
    println!("Withdraw circuit satisfied with {} constraints", cs.num_constraints());
}

#[test]
fn test_circuit_new_item() {
    // Start with empty tree
    let mut tree = SparseMerkleTree::new(DEFAULT_DEPTH);

    let old_root = tree.root();
    let proof = tree.get_proof(42); // Get proof for empty slot

    // Add new item
    tree.update(42, 100);
    let new_root = tree.root();

    let cs = ConstraintSystem::<Fr>::new_ref();

    let old_root_var = FpVar::new_input(cs.clone(), || Ok(old_root)).unwrap();
    let new_root_var = FpVar::new_input(cs.clone(), || Ok(new_root)).unwrap();
    let item_id_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(42u64))).unwrap();
    let old_qty_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(0u64))).unwrap(); // Was empty
    let new_qty_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(100u64))).unwrap();
    let proof_var = MerkleProofVar::new_witness(cs.clone(), &proof).unwrap();

    let computed_new_root = verify_and_update(
        cs.clone(),
        &old_root_var,
        &item_id_var,
        &old_qty_var,
        &new_qty_var,
        &proof_var,
    ).unwrap();

    computed_new_root.enforce_equal(&new_root_var).unwrap();

    assert!(cs.is_satisfied().unwrap());
    println!("New item circuit satisfied with {} constraints", cs.num_constraints());
}

#[test]
fn test_soundness_wrong_old_quantity() {
    let mut tree = SparseMerkleTree::from_items(&[(1, 100)], DEFAULT_DEPTH);

    let old_root = tree.root();
    let proof = tree.get_proof(1);

    tree.update(1, 150);
    let new_root = tree.root();

    let cs = ConstraintSystem::<Fr>::new_ref();

    let old_root_var = FpVar::new_input(cs.clone(), || Ok(old_root)).unwrap();
    let new_root_var = FpVar::new_input(cs.clone(), || Ok(new_root)).unwrap();
    let item_id_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(1u64))).unwrap();
    // WRONG old quantity!
    let old_qty_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(99u64))).unwrap();
    let new_qty_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(150u64))).unwrap();
    let proof_var = MerkleProofVar::new_witness(cs.clone(), &proof).unwrap();

    let computed_new_root = verify_and_update(
        cs.clone(),
        &old_root_var,
        &item_id_var,
        &old_qty_var,
        &new_qty_var,
        &proof_var,
    ).unwrap();

    computed_new_root.enforce_equal(&new_root_var).unwrap();

    // Should NOT be satisfied - wrong old quantity
    assert!(!cs.is_satisfied().unwrap());
}

#[test]
fn test_soundness_wrong_item_id() {
    let mut tree = SparseMerkleTree::from_items(&[(1, 100)], DEFAULT_DEPTH);

    let old_root = tree.root();
    let proof = tree.get_proof(1);

    tree.update(1, 150);
    let new_root = tree.root();

    let cs = ConstraintSystem::<Fr>::new_ref();

    let old_root_var = FpVar::new_input(cs.clone(), || Ok(old_root)).unwrap();
    let new_root_var = FpVar::new_input(cs.clone(), || Ok(new_root)).unwrap();
    // WRONG item ID!
    let item_id_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(2u64))).unwrap();
    let old_qty_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(100u64))).unwrap();
    let new_qty_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(150u64))).unwrap();
    let proof_var = MerkleProofVar::new_witness(cs.clone(), &proof).unwrap();

    let computed_new_root = verify_and_update(
        cs.clone(),
        &old_root_var,
        &item_id_var,
        &old_qty_var,
        &new_qty_var,
        &proof_var,
    ).unwrap();

    computed_new_root.enforce_equal(&new_root_var).unwrap();

    // Should NOT be satisfied - wrong item ID
    assert!(!cs.is_satisfied().unwrap());
}

#[test]
fn test_large_item_ids() {
    // Use item IDs near the max for depth 12 (0 to 4095)
    let items = vec![
        (0, 10),
        (1, 20),
        (4094, 30),
        (4095, 40),
    ];

    let tree = SparseMerkleTree::from_items(&items, DEFAULT_DEPTH);

    for &(item_id, quantity) in &items {
        let proof = tree.get_proof(item_id);
        assert!(tree.verify_proof(item_id, quantity, &proof));
    }
}

#[test]
#[should_panic(expected = "item_id exceeds tree capacity")]
fn test_item_id_overflow() {
    let mut tree = SparseMerkleTree::new(DEFAULT_DEPTH);

    // 4096 is out of bounds for depth 12 (max is 4095)
    tree.update(4096, 100);
}
