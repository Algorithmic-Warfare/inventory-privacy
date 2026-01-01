//! In-circuit SMT verification gadgets using Anemoi hash.
//!
//! These gadgets allow ZK circuits to verify Merkle proofs and update SMT roots.
//! Uses Anemoi for ~2x constraint reduction compared to Poseidon.

use ark_bn254::Fr;
use ark_r1cs_std::{
    prelude::*,
    fields::fp::FpVar,
    boolean::Boolean,
};
use ark_relations::r1cs::{ConstraintSystemRef, SynthesisError};

use anemoi::{anemoi_hash_two, anemoi_hash_two_var};
use super::proof::MerkleProof;

/// Compute the default leaf hash H(0, 0) natively using Anemoi.
/// This is the hash of an empty slot and is constant.
/// Precomputing this saves constraints per verify_and_update call.
pub fn compute_default_leaf_hash() -> Fr {
    anemoi_hash_two(Fr::from(0u64), Fr::from(0u64))
}

/// Circuit variable representation of a Merkle proof.
#[derive(Clone)]
pub struct MerkleProofVar {
    /// Sibling hashes as circuit variables
    path: Vec<FpVar<Fr>>,

    /// Direction booleans as circuit variables
    indices: Vec<Boolean<Fr>>,
}

impl MerkleProofVar {
    /// Allocate a Merkle proof as witness variables.
    pub fn new_witness(
        cs: ConstraintSystemRef<Fr>,
        proof: &MerkleProof<Fr>,
    ) -> Result<Self, SynthesisError> {
        let path = proof
            .path()
            .iter()
            .map(|h| FpVar::new_witness(cs.clone(), || Ok(*h)))
            .collect::<Result<Vec<_>, _>>()?;

        let indices = proof
            .indices()
            .iter()
            .map(|&b| Boolean::new_witness(cs.clone(), || Ok(b)))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self { path, indices })
    }

    /// Get the path variables.
    pub fn path(&self) -> &[FpVar<Fr>] {
        &self.path
    }

    /// Get the indices variables.
    pub fn indices(&self) -> &[Boolean<Fr>] {
        &self.indices
    }

    /// Get the proof depth.
    pub fn depth(&self) -> usize {
        self.path.len()
    }
}

/// Hash two field elements using Anemoi in-circuit.
pub fn hash_two(
    cs: ConstraintSystemRef<Fr>,
    left: &FpVar<Fr>,
    right: &FpVar<Fr>,
) -> Result<FpVar<Fr>, SynthesisError> {
    anemoi_hash_two_var(cs, left, right)
}

/// Hash a leaf (item_id, quantity) using Anemoi in-circuit.
pub fn hash_leaf(
    cs: ConstraintSystemRef<Fr>,
    item_id: &FpVar<Fr>,
    quantity: &FpVar<Fr>,
) -> Result<FpVar<Fr>, SynthesisError> {
    hash_two(cs, item_id, quantity)
}

/// Compute the root hash from a leaf and Merkle path in-circuit.
///
/// This is the core membership verification gadget.
pub fn compute_root_from_path(
    cs: ConstraintSystemRef<Fr>,
    leaf_hash: &FpVar<Fr>,
    proof: &MerkleProofVar,
) -> Result<FpVar<Fr>, SynthesisError> {
    let mut current = leaf_hash.clone();

    for (sibling, is_right) in proof.path.iter().zip(proof.indices.iter()) {
        // If is_right: H(sibling, current), else H(current, sibling)
        let left = is_right.select(sibling, &current)?;
        let right = is_right.select(&current, sibling)?;

        current = hash_two(cs.clone(), &left, &right)?;
    }

    Ok(current)
}

/// Verify that a leaf with given item_id and quantity exists in the tree with given root.
///
/// This constrains: compute_root(H(item_id, quantity), proof) == expected_root
pub fn verify_membership(
    cs: ConstraintSystemRef<Fr>,
    expected_root: &FpVar<Fr>,
    item_id: &FpVar<Fr>,
    quantity: &FpVar<Fr>,
    proof: &MerkleProofVar,
) -> Result<(), SynthesisError> {
    // Compute leaf hash
    let leaf_hash = hash_leaf(cs.clone(), item_id, quantity)?;

    // Compute root from proof
    let computed_root = compute_root_from_path(cs, &leaf_hash, proof)?;

    // Enforce equality
    computed_root.enforce_equal(expected_root)?;

    Ok(())
}

/// Verify membership and compute the new root after updating the leaf.
///
/// This is used for state transitions (deposit/withdraw).
///
/// Handles insertions specially: when old_quantity == 0, verifies against
/// the default leaf hash H(0, 0) instead of H(item_id, 0). This allows
/// adding new items to empty slots.
///
/// Returns the new root after setting the leaf to new_quantity.
pub fn verify_and_update(
    cs: ConstraintSystemRef<Fr>,
    old_root: &FpVar<Fr>,
    item_id: &FpVar<Fr>,
    old_quantity: &FpVar<Fr>,
    new_quantity: &FpVar<Fr>,
    proof: &MerkleProofVar,
) -> Result<FpVar<Fr>, SynthesisError> {
    // For insertions (old_quantity == 0), use precomputed default leaf hash H(0, 0)
    // For updates (old_quantity > 0), use regular hash H(item_id, old_quantity)
    let zero = FpVar::zero();
    let is_insertion = old_quantity.is_eq(&zero)?;

    // Use precomputed constant instead of computing hash_leaf(0, 0) in-circuit
    let default_leaf_hash_var = FpVar::constant(compute_default_leaf_hash());
    let regular_old_hash = hash_leaf(cs.clone(), item_id, old_quantity)?;

    let old_leaf_hash = is_insertion.select(&default_leaf_hash_var, &regular_old_hash)?;

    // Verify old state
    let computed_old_root = compute_root_from_path(cs.clone(), &old_leaf_hash, proof)?;
    computed_old_root.enforce_equal(old_root)?;

    // Compute new leaf hash (always uses item_id)
    let new_leaf_hash = hash_leaf(cs.clone(), item_id, new_quantity)?;

    // Compute new root using the same path (siblings unchanged)
    let new_root = compute_root_from_path(cs, &new_leaf_hash, proof)?;

    Ok(new_root)
}

/// Verify that an item is NOT in the tree (quantity = 0).
///
/// This proves non-membership by showing the leaf at item_id is empty.
pub fn verify_non_membership(
    cs: ConstraintSystemRef<Fr>,
    expected_root: &FpVar<Fr>,
    item_id: &FpVar<Fr>,
    proof: &MerkleProofVar,
) -> Result<(), SynthesisError> {
    let zero = FpVar::zero();
    verify_membership(cs, expected_root, item_id, &zero, proof)
}

#[cfg(test)]
mod gadget_tests {
    use super::*;
    use crate::smt::{SparseMerkleTree, DEFAULT_DEPTH};
    use ark_relations::r1cs::ConstraintSystem;

    #[test]
    fn test_verify_membership_valid() {
        let mut tree = SparseMerkleTree::new(DEFAULT_DEPTH);

        tree.update(1, 100);
        tree.update(42, 50);

        let root = tree.root();
        let proof = tree.get_proof(1);

        // Create constraint system
        let cs = ConstraintSystem::<Fr>::new_ref();

        // Allocate variables
        let root_var = FpVar::new_input(cs.clone(), || Ok(root)).unwrap();
        let item_id_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(1u64))).unwrap();
        let quantity_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(100u64))).unwrap();
        let proof_var = MerkleProofVar::new_witness(cs.clone(), &proof).unwrap();

        // Verify membership
        verify_membership(
            cs.clone(),
            &root_var,
            &item_id_var,
            &quantity_var,
            &proof_var,
        )
        .unwrap();

        assert!(cs.is_satisfied().unwrap());
    }

    #[test]
    fn test_verify_membership_wrong_quantity() {
        let mut tree = SparseMerkleTree::new(DEFAULT_DEPTH);

        tree.update(1, 100);
        let root = tree.root();
        let proof = tree.get_proof(1);

        let cs = ConstraintSystem::<Fr>::new_ref();

        let root_var = FpVar::new_input(cs.clone(), || Ok(root)).unwrap();
        let item_id_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(1u64))).unwrap();
        // Wrong quantity!
        let quantity_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(99u64))).unwrap();
        let proof_var = MerkleProofVar::new_witness(cs.clone(), &proof).unwrap();

        verify_membership(
            cs.clone(),
            &root_var,
            &item_id_var,
            &quantity_var,
            &proof_var,
        )
        .unwrap();

        // Should NOT be satisfied - wrong quantity
        assert!(!cs.is_satisfied().unwrap());
    }

    #[test]
    fn test_verify_and_update() {
        let mut tree = SparseMerkleTree::new(DEFAULT_DEPTH);

        tree.update(1, 100);
        let old_root = tree.root();
        let proof = tree.get_proof(1);

        // Compute expected new root natively
        tree.update(1, 150);
        let expected_new_root = tree.root();

        let cs = ConstraintSystem::<Fr>::new_ref();

        let old_root_var = FpVar::new_input(cs.clone(), || Ok(old_root)).unwrap();
        let item_id_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(1u64))).unwrap();
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
        )
        .unwrap();

        // Verify computed root matches expected
        let expected_var = FpVar::new_input(cs.clone(), || Ok(expected_new_root)).unwrap();
        computed_new_root.enforce_equal(&expected_var).unwrap();

        assert!(cs.is_satisfied().unwrap());
    }

    #[test]
    fn test_constraint_count() {
        let mut tree = SparseMerkleTree::new(DEFAULT_DEPTH);

        tree.update(1, 100);
        let root = tree.root();
        let proof = tree.get_proof(1);

        let cs = ConstraintSystem::<Fr>::new_ref();

        let root_var = FpVar::new_input(cs.clone(), || Ok(root)).unwrap();
        let item_id_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(1u64))).unwrap();
        let quantity_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(100u64))).unwrap();
        let proof_var = MerkleProofVar::new_witness(cs.clone(), &proof).unwrap();

        verify_membership(
            cs.clone(),
            &root_var,
            &item_id_var,
            &quantity_var,
            &proof_var,
        )
        .unwrap();

        let num_constraints = cs.num_constraints();
        println!("Anemoi SMT membership verification constraints (depth {}): {}", DEFAULT_DEPTH, num_constraints);

        // With Anemoi and depth 12, expect roughly:
        // - 1 leaf hash: ~126 constraints
        // - 12 node hashes: 12 * 126 = 1512 constraints
        // Total: ~1638 constraints (vs ~3900 with Poseidon)
        assert!(
            num_constraints < 2500,
            "Too many constraints: {}. Expected < 2500 for depth {}",
            num_constraints,
            DEFAULT_DEPTH
        );
    }
}
