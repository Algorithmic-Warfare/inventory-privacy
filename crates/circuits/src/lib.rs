//! ZK circuits for private inventory operations using Sparse Merkle Trees.
//!
//! This crate provides SMT-based circuits for:
//! - `StateTransitionCircuit`: Prove valid deposit/withdraw with capacity checking
//! - `ItemExistsSMTCircuit`: Prove inventory contains >= N of item X
//! - `CapacitySMTCircuit`: Prove inventory volume is within capacity

// Core modules
pub mod commitment; // Poseidon config (shared)
pub mod range_check; // Range checks for underflow prevention
pub mod signal;
pub mod smt;
pub mod smt_commitment;

// Circuit modules
pub mod capacity_smt;
pub mod item_exists_smt;
pub mod state_transition;

#[cfg(test)]
mod tests;

// Poseidon configuration (shared utility)
pub use commitment::poseidon_config;

// SMT infrastructure
pub use smt::{
    compute_root_from_path, verify_and_update, verify_membership, MerkleProof, MerkleProofVar,
    SparseMerkleTree, DEFAULT_DEPTH,
};

// Signal hash (public input compression)
pub use signal::{
    compute_signal_hash, compute_signal_hash_var, OpType, SignalInputs, SignalInputsVar,
};

// SMT commitment
pub use smt_commitment::{
    create_smt_commitment, create_smt_commitment_var, InventoryState, InventoryStateVar,
};

// Circuit exports
pub use state_transition::StateTransitionCircuit;
pub use item_exists_smt::{compute_item_exists_hash, ItemExistsSMTCircuit};
pub use capacity_smt::{compute_capacity_hash, CapacitySMTCircuit};

use ark_bn254::Fr;

/// Common type aliases
pub type ConstraintF = Fr;
