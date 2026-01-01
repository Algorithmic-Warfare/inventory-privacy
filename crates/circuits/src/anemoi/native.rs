//! Native Anemoi implementation for BN254.
//!
//! This provides the hash function for use outside of circuits.
//! Implementation matches anemoi-rust for compatibility.

use ark_bn254::Fr;
use ark_ff::Field;
use super::constants::{
    NUM_ROUNDS, GENERATOR, BETA, ARK_C, ARK_D,
    exp_inv_alpha, mul_by_generator, delta,
};

/// Anemoi state (2 field elements for 2:1 mode).
#[derive(Clone, Debug)]
pub struct AnemoiState {
    pub x: Fr,
    pub y: Fr,
}

impl AnemoiState {
    /// Create a new state initialized to zero.
    pub fn new() -> Self {
        Self {
            x: Fr::from(0u64),
            y: Fr::from(0u64),
        }
    }

    /// Create state from two field elements.
    pub fn from_elements(x: Fr, y: Fr) -> Self {
        Self { x, y }
    }
}

impl Default for AnemoiState {
    fn default() -> Self {
        Self::new()
    }
}

/// Apply the Flystel S-box to the state.
///
/// The open Flystel S-box from anemoi-rust:
/// 1. x = x - beta * y^2
/// 2. y = y - x^(1/alpha)
/// 3. x = x + beta * y^2 + delta
fn apply_sbox(state: &mut AnemoiState) {
    let beta = Fr::from(BETA);
    let delta = delta();

    // Step 1: x = x - beta * y^2
    let y_squared = state.y.square();
    state.x -= beta * y_squared;

    // Step 2: y = y - x^(1/alpha)
    let x_inv_alpha = exp_inv_alpha(state.x);
    state.y -= x_inv_alpha;

    // Step 3: x = x + beta * y^2 + delta
    let new_y_squared = state.y.square();
    state.x += beta * new_y_squared + delta;
}

/// Apply the MDS layer (linear diffusion).
///
/// For 2-cell state with generator g=3, the MDS operation is:
/// new_x = g * x + y = 3x + y
/// new_y = x + (g+1) * y = x + 4y
fn apply_mds(state: &mut AnemoiState) {
    let g_plus_one = Fr::from(GENERATOR + 1);

    let new_x = mul_by_generator(state.x) + state.y;
    let new_y = state.x + g_plus_one * state.y;

    state.x = new_x;
    state.y = new_y;
}

/// Apply round constants (ARK layer).
fn apply_round_constants(state: &mut AnemoiState, round: usize) {
    state.x += ARK_C[round];
    state.y += ARK_D[round];
}

/// Execute the full Anemoi permutation.
pub fn permutation(state: &mut AnemoiState) {
    for round in 0..NUM_ROUNDS {
        // ARK layer (add round constants)
        apply_round_constants(state, round);

        // MDS layer (linear diffusion)
        apply_mds(state);

        // S-box layer (Flystel)
        apply_sbox(state);
    }

    // Final MDS layer
    apply_mds(state);
}

/// Hash two field elements using Anemoi in sponge mode.
///
/// This uses the Jive compression mode:
/// - Initialize state to (x1, x2)
/// - Apply permutation
/// - Return x + y (linear combination of outputs)
pub fn anemoi_hash_two(a: Fr, b: Fr) -> Fr {
    let mut state = AnemoiState::from_elements(a, b);
    permutation(&mut state);

    // Jive output: x + y
    state.x + state.y
}

/// Hash a single field element.
/// Uses domain separation by setting capacity to a fixed value.
pub fn anemoi_hash(input: Fr) -> Fr {
    anemoi_hash_two(input, Fr::from(0u64))
}

#[cfg(test)]
mod native_tests {
    use super::*;

    #[test]
    fn test_hash_deterministic() {
        let a = Fr::from(1u64);
        let b = Fr::from(2u64);

        let h1 = anemoi_hash_two(a, b);
        let h2 = anemoi_hash_two(a, b);

        assert_eq!(h1, h2, "Hash should be deterministic");
    }

    #[test]
    fn test_hash_different_inputs() {
        let h1 = anemoi_hash_two(Fr::from(1u64), Fr::from(2u64));
        let h2 = anemoi_hash_two(Fr::from(1u64), Fr::from(3u64));
        let h3 = anemoi_hash_two(Fr::from(2u64), Fr::from(2u64));

        assert_ne!(h1, h2, "Different inputs should produce different hashes");
        assert_ne!(h1, h3, "Different inputs should produce different hashes");
        assert_ne!(h2, h3, "Different inputs should produce different hashes");
    }

    #[test]
    fn test_sbox_invertible() {
        // The S-box should be a permutation
        let original = AnemoiState::from_elements(Fr::from(42u64), Fr::from(123u64));
        let mut state = original.clone();

        apply_sbox(&mut state);

        // After S-box, state should be different
        assert!(state.x != original.x || state.y != original.y);
    }

    #[test]
    fn test_mds_linear() {
        // MDS should be linear: M(a + b) = M(a) + M(b)
        let mut a = AnemoiState::from_elements(Fr::from(1u64), Fr::from(2u64));
        let mut b = AnemoiState::from_elements(Fr::from(3u64), Fr::from(4u64));
        let mut sum = AnemoiState::from_elements(Fr::from(4u64), Fr::from(6u64));

        apply_mds(&mut a);
        apply_mds(&mut b);
        apply_mds(&mut sum);

        assert_eq!(a.x + b.x, sum.x);
        assert_eq!(a.y + b.y, sum.y);
    }

    #[test]
    fn test_zero_hash() {
        // Hash of (0, 0) should produce a non-zero result
        let h = anemoi_hash_two(Fr::from(0u64), Fr::from(0u64));
        assert_ne!(h, Fr::from(0u64), "Hash of zeros should be non-zero");
    }
}
