//! Local proof verification for testing SMT-based circuits.

use ark_bn254::{Bn254, Fr};
use ark_groth16::{Groth16, Proof, VerifyingKey};
use ark_snark::SNARK;
use thiserror::Error;

/// Errors during verification
#[derive(Error, Debug)]
pub enum VerifyError {
    #[error("Verification failed: {0}")]
    Verification(String),
    #[error("Invalid public inputs")]
    InvalidInputs,
}

/// Verify a StateTransition proof (uses signal hash as single public input)
pub fn verify_state_transition(
    vk: &VerifyingKey<Bn254>,
    proof: &Proof<Bn254>,
    signal_hash: Fr,
) -> Result<bool, VerifyError> {
    let public_inputs = vec![signal_hash];

    Groth16::<Bn254>::verify(vk, &public_inputs, proof)
        .map_err(|e| VerifyError::Verification(e.to_string()))
}

/// Verify an ItemExists proof (uses public hash as single input)
pub fn verify_item_exists(
    vk: &VerifyingKey<Bn254>,
    proof: &Proof<Bn254>,
    public_hash: Fr,
) -> Result<bool, VerifyError> {
    let public_inputs = vec![public_hash];

    Groth16::<Bn254>::verify(vk, &public_inputs, proof)
        .map_err(|e| VerifyError::Verification(e.to_string()))
}

/// Verify a Capacity proof (uses public hash as single input)
pub fn verify_capacity(
    vk: &VerifyingKey<Bn254>,
    proof: &Proof<Bn254>,
    public_hash: Fr,
) -> Result<bool, VerifyError> {
    let public_inputs = vec![public_hash];

    Groth16::<Bn254>::verify(vk, &public_inputs, proof)
        .map_err(|e| VerifyError::Verification(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prove::{prove_capacity, prove_item_exists, InventoryState};
    use crate::setup::{setup_capacity, setup_item_exists};
    use ark_std::rand::{rngs::StdRng, SeedableRng};

    #[test]
    fn test_verify_item_exists() {
        let mut rng = StdRng::seed_from_u64(42);
        let keys = setup_item_exists(&mut rng).unwrap();

        // Create inventory with item
        let blinding = Fr::from(12345u64);
        let mut state = InventoryState::new(blinding);
        state.tree.update(42, 100);
        state.current_volume = 500;

        // Generate proof
        let proof_result = prove_item_exists(&keys.proving_key, &state, 42, 50).unwrap();

        // Verify with correct public hash
        let valid = verify_item_exists(
            &keys.verifying_key,
            &proof_result.proof,
            proof_result.public_inputs[0],
        )
        .unwrap();

        assert!(valid);
    }

    #[test]
    fn test_verify_wrong_hash_fails() {
        let mut rng = StdRng::seed_from_u64(42);
        let keys = setup_item_exists(&mut rng).unwrap();

        // Create inventory with item
        let blinding = Fr::from(12345u64);
        let mut state = InventoryState::new(blinding);
        state.tree.update(42, 100);
        state.current_volume = 500;

        // Generate proof
        let proof_result = prove_item_exists(&keys.proving_key, &state, 42, 50).unwrap();

        // Try to verify with wrong public hash
        let wrong_hash = Fr::from(99999u64);
        let valid = verify_item_exists(&keys.verifying_key, &proof_result.proof, wrong_hash).unwrap();

        assert!(!valid);
    }

    #[test]
    fn test_verify_capacity() {
        let mut rng = StdRng::seed_from_u64(42);
        let keys = setup_capacity(&mut rng).unwrap();

        let blinding = Fr::from(12345u64);
        let mut state = InventoryState::new(blinding);
        state.tree.update(1, 100);
        state.current_volume = 500;

        let proof_result = prove_capacity(&keys.proving_key, &state, 1000).unwrap();

        let valid = verify_capacity(
            &keys.verifying_key,
            &proof_result.proof,
            proof_result.public_inputs[0],
        )
        .unwrap();

        assert!(valid);
    }
}
