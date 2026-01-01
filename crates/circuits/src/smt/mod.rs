//! Sparse Merkle Tree implementation for inventory privacy circuits.
//!
//! This module provides:
//! - Native SMT operations (insert, update, proof generation)
//! - In-circuit SMT verification gadgets (Poseidon-based)
//! - Anemoi-based verification gadgets (~2x fewer constraints)
//! - Merkle proof structures

mod tree;
mod proof;
mod gadgets;
pub mod anemoi_gadgets;

#[cfg(test)]
mod tests;

pub use tree::{SparseMerkleTree, DEFAULT_DEPTH};
pub use proof::MerkleProof;
pub use gadgets::{
    MerkleProofVar, verify_membership, verify_and_update, compute_root_from_path,
    compute_default_leaf_hash,
};
pub use anemoi_gadgets::{
    verify_membership_anemoi, verify_and_update_anemoi, compute_root_from_path_anemoi,
    default_leaf_hash_anemoi,
    MerkleProofVar as AnemoiMerkleProofVar,
};
