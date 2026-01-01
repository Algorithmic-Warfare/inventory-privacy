//! Anemoi hash function implementation for BN254.
//!
//! Anemoi is a ZK-friendly hash function that achieves ~2x fewer R1CS constraints
//! compared to Poseidon for equivalent security. This module provides:
//!
//! - Native Anemoi hash (for computing hashes outside circuits)
//! - R1CS gadgets (for in-circuit verification)
//!
//! Reference: "New Design Techniques for Efficient Arithmetization-Oriented Hash Functions:
//! Anemoi Permutations and Jive Compression Mode" (CRYPTO 2023)
//! https://eprint.iacr.org/2022/840

mod constants;
mod native;
mod gadgets;

#[cfg(test)]
mod tests;

pub use native::{anemoi_hash, anemoi_hash_two, anemoi_hash_many, AnemoiState};
pub use gadgets::{anemoi_hash_var, anemoi_hash_two_var, anemoi_hash_many_var};
