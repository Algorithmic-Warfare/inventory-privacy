//! Anemoi-based SMT verification gadgets.
//!
//! These gadgets provide Anemoi hash alternatives for SMT operations,
//! offering ~2x fewer constraints compared to Poseidon-based gadgets.

use ark_bn254::Fr;
use ark_r1cs_std::{
    prelude::*,
    fields::fp::FpVar,
    boolean::Boolean,
};
use ark_relations::r1cs::{ConstraintSystemRef, SynthesisError};

use crate::anemoi::{anemoi_hash_two, anemoi_hash_two_var};
use super::proof::MerkleProof;

/// Precomputed default leaf hash H(0, 0) using Anemoi.
/// This is constant and can be used across all circuits.
pub fn default_leaf_hash_anemoi() -> Fr {
    anemoi_hash_two(Fr::from(0u64), Fr::from(0u64))
}

/// Circuit variable representation of a Merkle proof (same as Poseidon version).
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
pub fn hash_two_anemoi(
    cs: ConstraintSystemRef<Fr>,
    left: &FpVar<Fr>,
    right: &FpVar<Fr>,
) -> Result<FpVar<Fr>, SynthesisError> {
    anemoi_hash_two_var(cs, left, right)
}

/// Hash a leaf (item_id, quantity) using Anemoi in-circuit.
pub fn hash_leaf_anemoi(
    cs: ConstraintSystemRef<Fr>,
    item_id: &FpVar<Fr>,
    quantity: &FpVar<Fr>,
) -> Result<FpVar<Fr>, SynthesisError> {
    hash_two_anemoi(cs, item_id, quantity)
}

/// Compute the root hash from a leaf and Merkle path in-circuit using Anemoi.
pub fn compute_root_from_path_anemoi(
    cs: ConstraintSystemRef<Fr>,
    leaf_hash: &FpVar<Fr>,
    proof: &MerkleProofVar,
) -> Result<FpVar<Fr>, SynthesisError> {
    let mut current = leaf_hash.clone();

    for (sibling, is_right) in proof.path.iter().zip(proof.indices.iter()) {
        // If is_right: H(sibling, current), else H(current, sibling)
        let left = is_right.select(sibling, &current)?;
        let right = is_right.select(&current, sibling)?;

        current = hash_two_anemoi(cs.clone(), &left, &right)?;
    }

    Ok(current)
}

/// Verify that a leaf with given item_id and quantity exists in the tree with given root.
pub fn verify_membership_anemoi(
    cs: ConstraintSystemRef<Fr>,
    expected_root: &FpVar<Fr>,
    item_id: &FpVar<Fr>,
    quantity: &FpVar<Fr>,
    proof: &MerkleProofVar,
) -> Result<(), SynthesisError> {
    // Compute leaf hash
    let leaf_hash = hash_leaf_anemoi(cs.clone(), item_id, quantity)?;

    // Compute root from proof
    let computed_root = compute_root_from_path_anemoi(cs, &leaf_hash, proof)?;

    // Enforce equality
    computed_root.enforce_equal(expected_root)?;

    Ok(())
}

/// Verify membership and compute the new root after updating the leaf using Anemoi.
///
/// Handles insertions specially: when old_quantity == 0, verifies against
/// the default leaf hash H(0, 0) instead of H(item_id, 0).
///
/// Returns the new root after setting the leaf to new_quantity.
pub fn verify_and_update_anemoi(
    cs: ConstraintSystemRef<Fr>,
    old_root: &FpVar<Fr>,
    item_id: &FpVar<Fr>,
    old_quantity: &FpVar<Fr>,
    new_quantity: &FpVar<Fr>,
    proof: &MerkleProofVar,
) -> Result<FpVar<Fr>, SynthesisError> {
    // For insertions (old_quantity == 0), use precomputed default leaf hash H(0, 0)
    let zero = FpVar::zero();
    let is_insertion = old_quantity.is_eq(&zero)?;

    // Use precomputed constant
    let default_leaf_hash_var = FpVar::constant(default_leaf_hash_anemoi());
    let regular_old_hash = hash_leaf_anemoi(cs.clone(), item_id, old_quantity)?;

    let old_leaf_hash = is_insertion.select(&default_leaf_hash_var, &regular_old_hash)?;

    // Verify old state
    let computed_old_root = compute_root_from_path_anemoi(cs.clone(), &old_leaf_hash, proof)?;
    computed_old_root.enforce_equal(old_root)?;

    // Compute new leaf hash (always uses item_id)
    let new_leaf_hash = hash_leaf_anemoi(cs.clone(), item_id, new_quantity)?;

    // Compute new root using the same path
    let new_root = compute_root_from_path_anemoi(cs, &new_leaf_hash, proof)?;

    Ok(new_root)
}

#[cfg(test)]
mod anemoi_gadget_tests {
    use super::*;
    use crate::smt::DEFAULT_DEPTH;
    use ark_relations::r1cs::ConstraintSystem;

    // Simple native Anemoi-based SMT for testing
    fn compute_anemoi_tree_root(item_id: u64, quantity: u64, depth: usize) -> Fr {
        let mut current = anemoi_hash_two(Fr::from(item_id), Fr::from(quantity));
        let default_hash = default_leaf_hash_anemoi();

        // Compute path to root (item_id determines left/right at each level)
        let mut idx = item_id;
        for _ in 0..depth {
            if idx & 1 == 0 {
                // Current is left child
                current = anemoi_hash_two(current, default_hash);
            } else {
                // Current is right child
                current = anemoi_hash_two(default_hash, current);
            }
            idx >>= 1;
        }
        current
    }

    fn create_simple_proof(item_id: u64, depth: usize) -> MerkleProof<Fr> {
        let default_hash = default_leaf_hash_anemoi();
        let mut path = Vec::with_capacity(depth);
        let mut indices = Vec::with_capacity(depth);

        let mut idx = item_id;
        for _ in 0..depth {
            path.push(default_hash);
            indices.push((idx & 1) == 1);
            idx >>= 1;
        }

        MerkleProof::new(path, indices)
    }

    #[test]
    fn test_verify_membership_anemoi() {
        let item_id = 1u64;
        let quantity = 100u64;
        let depth = DEFAULT_DEPTH;

        let root = compute_anemoi_tree_root(item_id, quantity, depth);
        let proof = create_simple_proof(item_id, depth);

        let cs = ConstraintSystem::<Fr>::new_ref();

        let root_var = FpVar::new_input(cs.clone(), || Ok(root)).unwrap();
        let item_id_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(item_id))).unwrap();
        let quantity_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(quantity))).unwrap();
        let proof_var = MerkleProofVar::new_witness(cs.clone(), &proof).unwrap();

        verify_membership_anemoi(
            cs.clone(),
            &root_var,
            &item_id_var,
            &quantity_var,
            &proof_var,
        )
        .unwrap();

        assert!(cs.is_satisfied().unwrap());

        let num_constraints = cs.num_constraints();
        println!("Anemoi SMT membership verification constraints (depth {}): {}", depth, num_constraints);
    }

    #[test]
    fn test_verify_membership_wrong_quantity() {
        let item_id = 1u64;
        let quantity = 100u64;
        let depth = DEFAULT_DEPTH;

        let root = compute_anemoi_tree_root(item_id, quantity, depth);
        let proof = create_simple_proof(item_id, depth);

        let cs = ConstraintSystem::<Fr>::new_ref();

        let root_var = FpVar::new_input(cs.clone(), || Ok(root)).unwrap();
        let item_id_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(item_id))).unwrap();
        // Wrong quantity!
        let quantity_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(99u64))).unwrap();
        let proof_var = MerkleProofVar::new_witness(cs.clone(), &proof).unwrap();

        verify_membership_anemoi(
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
    fn test_constraint_count_comparison() {
        let item_id = 1u64;
        let quantity = 100u64;
        let depth = DEFAULT_DEPTH;

        let root = compute_anemoi_tree_root(item_id, quantity, depth);
        let proof = create_simple_proof(item_id, depth);

        let cs = ConstraintSystem::<Fr>::new_ref();

        let root_var = FpVar::new_input(cs.clone(), || Ok(root)).unwrap();
        let item_id_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(item_id))).unwrap();
        let quantity_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(quantity))).unwrap();
        let proof_var = MerkleProofVar::new_witness(cs.clone(), &proof).unwrap();

        verify_membership_anemoi(
            cs.clone(),
            &root_var,
            &item_id_var,
            &quantity_var,
            &proof_var,
        )
        .unwrap();

        let anemoi_constraints = cs.num_constraints();

        // Compare to expected Poseidon constraints
        // Poseidon: ~300 per hash * (1 leaf + 12 nodes) = ~3900 constraints
        // Anemoi: ~126 per hash * (1 leaf + 12 nodes) = ~1638 constraints
        println!("Anemoi SMT constraints: {}", anemoi_constraints);
        println!("Expected Poseidon SMT constraints: ~3900");
        println!("Improvement: {:.1}x", 3900.0 / anemoi_constraints as f64);

        // Verify Anemoi is significantly better
        assert!(
            anemoi_constraints < 2500,
            "Expected < 2500 constraints for depth {}, got {}",
            depth,
            anemoi_constraints
        );
    }

    #[test]
    fn test_verify_and_update_anemoi() {
        let item_id = 1u64;
        let old_quantity = 100u64;
        let new_quantity = 150u64;
        let depth = DEFAULT_DEPTH;

        let old_root = compute_anemoi_tree_root(item_id, old_quantity, depth);
        let new_root = compute_anemoi_tree_root(item_id, new_quantity, depth);
        let proof = create_simple_proof(item_id, depth);

        let cs = ConstraintSystem::<Fr>::new_ref();

        let old_root_var = FpVar::new_input(cs.clone(), || Ok(old_root)).unwrap();
        let item_id_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(item_id))).unwrap();
        let old_qty_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(old_quantity))).unwrap();
        let new_qty_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(new_quantity))).unwrap();
        let proof_var = MerkleProofVar::new_witness(cs.clone(), &proof).unwrap();

        let computed_new_root = verify_and_update_anemoi(
            cs.clone(),
            &old_root_var,
            &item_id_var,
            &old_qty_var,
            &new_qty_var,
            &proof_var,
        )
        .unwrap();

        let expected_var = FpVar::new_input(cs.clone(), || Ok(new_root)).unwrap();
        computed_new_root.enforce_equal(&expected_var).unwrap();

        assert!(cs.is_satisfied().unwrap());
    }
}
