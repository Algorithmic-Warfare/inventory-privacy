//! Local proof verification for testing.

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

/// Verify an ItemExists proof
pub fn verify_item_exists(
    vk: &VerifyingKey<Bn254>,
    proof: &Proof<Bn254>,
    commitment: Fr,
    item_id: u32,
    min_quantity: u64,
) -> Result<bool, VerifyError> {
    let public_inputs = vec![commitment, Fr::from(item_id as u64), Fr::from(min_quantity)];

    Groth16::<Bn254>::verify(vk, &public_inputs, proof)
        .map_err(|e| VerifyError::Verification(e.to_string()))
}

/// Verify a Withdraw proof
pub fn verify_withdraw(
    vk: &VerifyingKey<Bn254>,
    proof: &Proof<Bn254>,
    old_commitment: Fr,
    new_commitment: Fr,
    item_id: u32,
    amount: u64,
) -> Result<bool, VerifyError> {
    let public_inputs = vec![
        old_commitment,
        new_commitment,
        Fr::from(item_id as u64),
        Fr::from(amount),
    ];

    Groth16::<Bn254>::verify(vk, &public_inputs, proof)
        .map_err(|e| VerifyError::Verification(e.to_string()))
}

/// Verify a Deposit proof
pub fn verify_deposit(
    vk: &VerifyingKey<Bn254>,
    proof: &Proof<Bn254>,
    old_commitment: Fr,
    new_commitment: Fr,
    item_id: u32,
    amount: u64,
) -> Result<bool, VerifyError> {
    let public_inputs = vec![
        old_commitment,
        new_commitment,
        Fr::from(item_id as u64),
        Fr::from(amount),
    ];

    Groth16::<Bn254>::verify(vk, &public_inputs, proof)
        .map_err(|e| VerifyError::Verification(e.to_string()))
}

/// Verify a Transfer proof
#[allow(clippy::too_many_arguments)]
pub fn verify_transfer(
    vk: &VerifyingKey<Bn254>,
    proof: &Proof<Bn254>,
    src_old_commitment: Fr,
    src_new_commitment: Fr,
    dst_old_commitment: Fr,
    dst_new_commitment: Fr,
    item_id: u32,
    amount: u64,
) -> Result<bool, VerifyError> {
    let public_inputs = vec![
        src_old_commitment,
        src_new_commitment,
        dst_old_commitment,
        dst_new_commitment,
        Fr::from(item_id as u64),
        Fr::from(amount),
    ];

    Groth16::<Bn254>::verify(vk, &public_inputs, proof)
        .map_err(|e| VerifyError::Verification(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prove::prove_item_exists;
    use crate::setup::setup_item_exists;
    use ark_std::rand::{rngs::StdRng, SeedableRng};
    use inventory_circuits::{commitment::poseidon_config, Inventory};
    use std::sync::Arc;

    #[test]
    fn test_verify_item_exists() {
        let mut rng = StdRng::seed_from_u64(42);
        let config = Arc::new(poseidon_config::<Fr>());
        let keys = setup_item_exists(&mut rng, config.clone()).unwrap();

        let inventory = Inventory::from_items(&[(1, 100)]);
        let blinding = Fr::from(12345u64);
        let commitment =
            inventory_circuits::commitment::create_inventory_commitment(&inventory, blinding, &config);

        let proof_result =
            prove_item_exists(&keys.proving_key, &inventory, blinding, 1, 50).unwrap();

        let valid = verify_item_exists(
            &keys.verifying_key,
            &proof_result.proof,
            commitment,
            1,
            50,
        )
        .unwrap();

        assert!(valid);
    }

    #[test]
    fn test_verify_wrong_inputs_fails() {
        let mut rng = StdRng::seed_from_u64(42);
        let config = Arc::new(poseidon_config::<Fr>());
        let keys = setup_item_exists(&mut rng, config.clone()).unwrap();

        let inventory = Inventory::from_items(&[(1, 100)]);
        let blinding = Fr::from(12345u64);
        let commitment =
            inventory_circuits::commitment::create_inventory_commitment(&inventory, blinding, &config);

        let proof_result =
            prove_item_exists(&keys.proving_key, &inventory, blinding, 1, 50).unwrap();

        // Try to verify with wrong min_quantity
        let valid = verify_item_exists(
            &keys.verifying_key,
            &proof_result.proof,
            commitment,
            1,
            200, // Wrong! We proved >= 50
        )
        .unwrap();

        assert!(!valid);
    }
}
