//! Proof generation for inventory circuits.

use std::sync::Arc;

use ark_bn254::{Bn254, Fr};
use ark_groth16::{Groth16, Proof, ProvingKey};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use ark_snark::SNARK;
use ark_std::rand::{rngs::StdRng, SeedableRng};
use thiserror::Error;

use inventory_circuits::{
    commitment::{create_inventory_commitment, poseidon_config},
    DepositCircuit, Inventory, ItemExistsCircuit, TransferCircuit, WithdrawCircuit,
};

/// Errors during proof generation
#[derive(Error, Debug)]
pub enum ProveError {
    #[error("Proof generation failed: {0}")]
    ProofGeneration(String),
    #[error("Invalid inventory state: {0}")]
    InvalidState(String),
    #[error("Serialization failed: {0}")]
    Serialization(String),
}

/// A proof with its public inputs
#[derive(Clone)]
pub struct ProofWithInputs {
    pub proof: Proof<Bn254>,
    pub public_inputs: Vec<Fr>,
}

impl ProofWithInputs {
    /// Serialize proof to bytes
    pub fn serialize_proof(&self) -> Result<Vec<u8>, ProveError> {
        let mut bytes = Vec::new();
        self.proof
            .serialize_compressed(&mut bytes)
            .map_err(|e| ProveError::Serialization(e.to_string()))?;
        Ok(bytes)
    }

    /// Serialize public inputs to bytes (each Fr is 32 bytes)
    pub fn serialize_public_inputs(&self) -> Result<Vec<u8>, ProveError> {
        let mut bytes = Vec::new();
        for input in &self.public_inputs {
            input
                .serialize_compressed(&mut bytes)
                .map_err(|e| ProveError::Serialization(e.to_string()))?;
        }
        Ok(bytes)
    }

    /// Deserialize proof from bytes
    pub fn deserialize_proof(bytes: &[u8]) -> Result<Proof<Bn254>, ProveError> {
        Proof::deserialize_compressed(bytes).map_err(|e| ProveError::Serialization(e.to_string()))
    }
}

/// Generate proof for ItemExistsCircuit
pub fn prove_item_exists(
    pk: &ProvingKey<Bn254>,
    inventory: &Inventory,
    blinding: Fr,
    item_id: u32,
    min_quantity: u64,
) -> Result<ProofWithInputs, ProveError> {
    let config = Arc::new(poseidon_config::<Fr>());
    let commitment = create_inventory_commitment(inventory, blinding, &config);

    // Verify the claim is valid
    let actual_qty = inventory.get_quantity(item_id);
    if actual_qty < min_quantity {
        return Err(ProveError::InvalidState(format!(
            "Insufficient quantity: have {}, need {}",
            actual_qty, min_quantity
        )));
    }

    let circuit = ItemExistsCircuit::new(
        inventory.clone(),
        blinding,
        commitment,
        item_id,
        min_quantity,
        config,
    );

    let mut rng = StdRng::from_entropy();
    let proof = Groth16::<Bn254>::prove(pk, circuit, &mut rng)
        .map_err(|e| ProveError::ProofGeneration(e.to_string()))?;

    let public_inputs = vec![commitment, Fr::from(item_id as u64), Fr::from(min_quantity)];

    Ok(ProofWithInputs {
        proof,
        public_inputs,
    })
}

/// Generate proof for WithdrawCircuit
pub fn prove_withdraw(
    pk: &ProvingKey<Bn254>,
    old_inventory: &Inventory,
    old_blinding: Fr,
    new_blinding: Fr,
    item_id: u32,
    amount: u64,
) -> Result<(ProofWithInputs, Inventory, Fr), ProveError> {
    let config = Arc::new(poseidon_config::<Fr>());
    let old_commitment = create_inventory_commitment(old_inventory, old_blinding, &config);

    // Create new inventory state
    let mut new_inventory = old_inventory.clone();
    new_inventory
        .withdraw(item_id, amount)
        .map_err(|e| ProveError::InvalidState(e.to_string()))?;

    let new_commitment = create_inventory_commitment(&new_inventory, new_blinding, &config);

    let circuit = WithdrawCircuit::new(
        old_inventory.clone(),
        new_inventory.clone(),
        old_blinding,
        new_blinding,
        old_commitment,
        new_commitment,
        item_id,
        amount,
        config,
    );

    let mut rng = StdRng::from_entropy();
    let proof = Groth16::<Bn254>::prove(pk, circuit, &mut rng)
        .map_err(|e| ProveError::ProofGeneration(e.to_string()))?;

    let public_inputs = vec![
        old_commitment,
        new_commitment,
        Fr::from(item_id as u64),
        Fr::from(amount),
    ];

    Ok((
        ProofWithInputs {
            proof,
            public_inputs,
        },
        new_inventory,
        new_commitment,
    ))
}

/// Generate proof for DepositCircuit
pub fn prove_deposit(
    pk: &ProvingKey<Bn254>,
    old_inventory: &Inventory,
    old_blinding: Fr,
    new_blinding: Fr,
    item_id: u32,
    amount: u64,
) -> Result<(ProofWithInputs, Inventory, Fr), ProveError> {
    let config = Arc::new(poseidon_config::<Fr>());
    let old_commitment = create_inventory_commitment(old_inventory, old_blinding, &config);

    // Create new inventory state
    let mut new_inventory = old_inventory.clone();
    new_inventory
        .deposit(item_id, amount)
        .map_err(|e| ProveError::InvalidState(e.to_string()))?;

    let new_commitment = create_inventory_commitment(&new_inventory, new_blinding, &config);

    let circuit = DepositCircuit::new(
        old_inventory.clone(),
        new_inventory.clone(),
        old_blinding,
        new_blinding,
        old_commitment,
        new_commitment,
        item_id,
        amount,
        config,
    );

    let mut rng = StdRng::from_entropy();
    let proof = Groth16::<Bn254>::prove(pk, circuit, &mut rng)
        .map_err(|e| ProveError::ProofGeneration(e.to_string()))?;

    let public_inputs = vec![
        old_commitment,
        new_commitment,
        Fr::from(item_id as u64),
        Fr::from(amount),
    ];

    Ok((
        ProofWithInputs {
            proof,
            public_inputs,
        },
        new_inventory,
        new_commitment,
    ))
}

/// Generate proof for TransferCircuit
#[allow(clippy::too_many_arguments)]
pub fn prove_transfer(
    pk: &ProvingKey<Bn254>,
    src_old_inventory: &Inventory,
    src_old_blinding: Fr,
    src_new_blinding: Fr,
    dst_old_inventory: &Inventory,
    dst_old_blinding: Fr,
    dst_new_blinding: Fr,
    item_id: u32,
    amount: u64,
) -> Result<(ProofWithInputs, Inventory, Fr, Inventory, Fr), ProveError> {
    let config = Arc::new(poseidon_config::<Fr>());

    // Compute old commitments
    let src_old_commitment =
        create_inventory_commitment(src_old_inventory, src_old_blinding, &config);
    let dst_old_commitment =
        create_inventory_commitment(dst_old_inventory, dst_old_blinding, &config);

    // Create new inventory states
    let mut src_new_inventory = src_old_inventory.clone();
    src_new_inventory
        .withdraw(item_id, amount)
        .map_err(|e| ProveError::InvalidState(format!("Source: {}", e)))?;

    let mut dst_new_inventory = dst_old_inventory.clone();
    dst_new_inventory
        .deposit(item_id, amount)
        .map_err(|e| ProveError::InvalidState(format!("Destination: {}", e)))?;

    // Compute new commitments
    let src_new_commitment =
        create_inventory_commitment(&src_new_inventory, src_new_blinding, &config);
    let dst_new_commitment =
        create_inventory_commitment(&dst_new_inventory, dst_new_blinding, &config);

    let circuit = TransferCircuit::new(
        src_old_inventory.clone(),
        src_new_inventory.clone(),
        src_old_blinding,
        src_new_blinding,
        dst_old_inventory.clone(),
        dst_new_inventory.clone(),
        dst_old_blinding,
        dst_new_blinding,
        src_old_commitment,
        src_new_commitment,
        dst_old_commitment,
        dst_new_commitment,
        item_id,
        amount,
        config,
    );

    let mut rng = StdRng::from_entropy();
    let proof = Groth16::<Bn254>::prove(pk, circuit, &mut rng)
        .map_err(|e| ProveError::ProofGeneration(e.to_string()))?;

    let public_inputs = vec![
        src_old_commitment,
        src_new_commitment,
        dst_old_commitment,
        dst_new_commitment,
        Fr::from(item_id as u64),
        Fr::from(amount),
    ];

    Ok((
        ProofWithInputs {
            proof,
            public_inputs,
        },
        src_new_inventory,
        src_new_commitment,
        dst_new_inventory,
        dst_new_commitment,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup::{setup_item_exists, setup_withdraw};
    use ark_std::rand::SeedableRng;

    #[test]
    fn test_prove_item_exists() {
        let mut rng = StdRng::seed_from_u64(42);
        let config = Arc::new(poseidon_config::<Fr>());
        let keys = setup_item_exists(&mut rng, config).unwrap();

        let inventory = Inventory::from_items(&[(1, 100), (2, 50)]);
        let blinding = Fr::from(12345u64);

        let result = prove_item_exists(&keys.proving_key, &inventory, blinding, 1, 50);
        assert!(result.is_ok());

        let proof_with_inputs = result.unwrap();
        assert_eq!(proof_with_inputs.public_inputs.len(), 3);
    }

    #[test]
    fn test_prove_withdraw() {
        let mut rng = StdRng::seed_from_u64(42);
        let config = Arc::new(poseidon_config::<Fr>());
        let keys = setup_withdraw(&mut rng, config).unwrap();

        let inventory = Inventory::from_items(&[(1, 100)]);
        let old_blinding = Fr::from(12345u64);
        let new_blinding = Fr::from(67890u64);

        let result = prove_withdraw(&keys.proving_key, &inventory, old_blinding, new_blinding, 1, 30);
        assert!(result.is_ok());

        let (proof_with_inputs, new_inventory, _) = result.unwrap();
        assert_eq!(proof_with_inputs.public_inputs.len(), 4);
        assert_eq!(new_inventory.get_quantity(1), 70);
    }
}
