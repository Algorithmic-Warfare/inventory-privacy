//! Proof generation library for private inventories.
//!
//! This crate provides utilities for:
//! - Trusted setup (generating proving and verifying keys)
//! - Proof generation for all circuit types
//! - Local proof verification (for testing)

pub mod prove;
pub mod setup;
pub mod verify;

pub use prove::{
    prove_deposit, prove_item_exists, prove_transfer, prove_withdraw, ProofWithInputs,
};
pub use setup::{setup_all_circuits, CircuitKeys, SetupError};
pub use verify::{verify_deposit, verify_item_exists, verify_transfer, verify_withdraw};

use ark_bn254::Fr;

/// Common field type for all operations
pub type ConstraintF = Fr;
