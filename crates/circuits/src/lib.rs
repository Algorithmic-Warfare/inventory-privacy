//! ZK circuits for private inventory operations.
//!
//! This crate provides circuits for:
//! - `ItemExistsCircuit`: Prove inventory contains >= N of item X
//! - `WithdrawCircuit`: Prove valid withdrawal with state transition
//! - `DepositCircuit`: Prove valid deposit with state transition
//! - `TransferCircuit`: Prove valid transfer between two inventories

pub mod commitment;
pub mod deposit;
pub mod inventory;
pub mod item_exists;
pub mod transfer;
pub mod withdraw;

#[cfg(test)]
mod tests;

pub use commitment::{create_inventory_commitment, poseidon_config};
pub use deposit::DepositCircuit;
pub use inventory::{Inventory, MAX_ITEM_SLOTS};
pub use item_exists::ItemExistsCircuit;
pub use transfer::TransferCircuit;
pub use withdraw::WithdrawCircuit;

use ark_bn254::Fr;

/// Common type aliases
pub type ConstraintF = Fr;
