//! Trusted setup utilities for generating proving and verifying keys.

use std::sync::Arc;

use ark_bn254::{Bn254, Fr};
use ark_groth16::{Groth16, ProvingKey, VerifyingKey};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use ark_snark::SNARK;
use ark_std::rand::{rngs::StdRng, SeedableRng};
use thiserror::Error;

use inventory_circuits::{
    commitment::poseidon_config, DepositCircuit, ItemExistsCircuit, TransferCircuit,
    WithdrawCircuit,
};

/// Errors that can occur during setup
#[derive(Error, Debug)]
pub enum SetupError {
    #[error("Circuit setup failed: {0}")]
    CircuitSetup(String),
    #[error("Serialization failed: {0}")]
    Serialization(String),
    #[error("Deserialization failed: {0}")]
    Deserialization(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Keys for a single circuit
#[derive(Clone)]
pub struct CircuitKeyPair {
    pub proving_key: ProvingKey<Bn254>,
    pub verifying_key: VerifyingKey<Bn254>,
}

impl CircuitKeyPair {
    /// Serialize proving key to bytes
    pub fn serialize_pk(&self) -> Result<Vec<u8>, SetupError> {
        let mut bytes = Vec::new();
        self.proving_key
            .serialize_compressed(&mut bytes)
            .map_err(|e| SetupError::Serialization(e.to_string()))?;
        Ok(bytes)
    }

    /// Serialize verifying key to bytes
    pub fn serialize_vk(&self) -> Result<Vec<u8>, SetupError> {
        let mut bytes = Vec::new();
        self.verifying_key
            .serialize_compressed(&mut bytes)
            .map_err(|e| SetupError::Serialization(e.to_string()))?;
        Ok(bytes)
    }

    /// Deserialize proving key from bytes
    pub fn deserialize_pk(bytes: &[u8]) -> Result<ProvingKey<Bn254>, SetupError> {
        ProvingKey::deserialize_compressed(bytes)
            .map_err(|e| SetupError::Deserialization(e.to_string()))
    }

    /// Deserialize verifying key from bytes
    pub fn deserialize_vk(bytes: &[u8]) -> Result<VerifyingKey<Bn254>, SetupError> {
        VerifyingKey::deserialize_compressed(bytes)
            .map_err(|e| SetupError::Deserialization(e.to_string()))
    }
}

/// All circuit keys
pub struct CircuitKeys {
    pub item_exists: CircuitKeyPair,
    pub withdraw: CircuitKeyPair,
    pub deposit: CircuitKeyPair,
    pub transfer: CircuitKeyPair,
}

impl CircuitKeys {
    /// Save all keys to a directory
    pub fn save_to_directory(&self, dir: &std::path::Path) -> Result<(), SetupError> {
        std::fs::create_dir_all(dir)?;

        std::fs::write(dir.join("item_exists.pk"), self.item_exists.serialize_pk()?)?;
        std::fs::write(dir.join("item_exists.vk"), self.item_exists.serialize_vk()?)?;

        std::fs::write(dir.join("withdraw.pk"), self.withdraw.serialize_pk()?)?;
        std::fs::write(dir.join("withdraw.vk"), self.withdraw.serialize_vk()?)?;

        std::fs::write(dir.join("deposit.pk"), self.deposit.serialize_pk()?)?;
        std::fs::write(dir.join("deposit.vk"), self.deposit.serialize_vk()?)?;

        std::fs::write(dir.join("transfer.pk"), self.transfer.serialize_pk()?)?;
        std::fs::write(dir.join("transfer.vk"), self.transfer.serialize_vk()?)?;

        Ok(())
    }

    /// Load all keys from a directory
    pub fn load_from_directory(dir: &std::path::Path) -> Result<Self, SetupError> {
        let item_exists = CircuitKeyPair {
            proving_key: CircuitKeyPair::deserialize_pk(&std::fs::read(
                dir.join("item_exists.pk"),
            )?)?,
            verifying_key: CircuitKeyPair::deserialize_vk(&std::fs::read(
                dir.join("item_exists.vk"),
            )?)?,
        };

        let withdraw = CircuitKeyPair {
            proving_key: CircuitKeyPair::deserialize_pk(&std::fs::read(dir.join("withdraw.pk"))?)?,
            verifying_key: CircuitKeyPair::deserialize_vk(&std::fs::read(
                dir.join("withdraw.vk"),
            )?)?,
        };

        let deposit = CircuitKeyPair {
            proving_key: CircuitKeyPair::deserialize_pk(&std::fs::read(dir.join("deposit.pk"))?)?,
            verifying_key: CircuitKeyPair::deserialize_vk(&std::fs::read(dir.join("deposit.vk"))?)?,
        };

        let transfer = CircuitKeyPair {
            proving_key: CircuitKeyPair::deserialize_pk(&std::fs::read(dir.join("transfer.pk"))?)?,
            verifying_key: CircuitKeyPair::deserialize_vk(&std::fs::read(
                dir.join("transfer.vk"),
            )?)?,
        };

        Ok(Self {
            item_exists,
            withdraw,
            deposit,
            transfer,
        })
    }
}

/// Run trusted setup for all circuits
pub fn setup_all_circuits() -> Result<CircuitKeys, SetupError> {
    // Use a fixed seed for reproducible setup (in production, use secure randomness)
    let mut rng = StdRng::seed_from_u64(42);
    let config = Arc::new(poseidon_config::<Fr>());

    println!("Setting up ItemExistsCircuit...");
    let item_exists = setup_item_exists(&mut rng, config.clone())?;

    println!("Setting up WithdrawCircuit...");
    let withdraw = setup_withdraw(&mut rng, config.clone())?;

    println!("Setting up DepositCircuit...");
    let deposit = setup_deposit(&mut rng, config.clone())?;

    println!("Setting up TransferCircuit...");
    let transfer = setup_transfer(&mut rng, config)?;

    Ok(CircuitKeys {
        item_exists,
        withdraw,
        deposit,
        transfer,
    })
}

/// Setup ItemExistsCircuit
pub fn setup_item_exists(
    rng: &mut StdRng,
    config: Arc<ark_crypto_primitives::sponge::poseidon::PoseidonConfig<Fr>>,
) -> Result<CircuitKeyPair, SetupError> {
    let circuit = ItemExistsCircuit::empty(config);
    let (pk, vk) = Groth16::<Bn254>::circuit_specific_setup(circuit, rng)
        .map_err(|e| SetupError::CircuitSetup(e.to_string()))?;

    Ok(CircuitKeyPair {
        proving_key: pk,
        verifying_key: vk,
    })
}

/// Setup WithdrawCircuit
pub fn setup_withdraw(
    rng: &mut StdRng,
    config: Arc<ark_crypto_primitives::sponge::poseidon::PoseidonConfig<Fr>>,
) -> Result<CircuitKeyPair, SetupError> {
    let circuit = WithdrawCircuit::empty(config);
    let (pk, vk) = Groth16::<Bn254>::circuit_specific_setup(circuit, rng)
        .map_err(|e| SetupError::CircuitSetup(e.to_string()))?;

    Ok(CircuitKeyPair {
        proving_key: pk,
        verifying_key: vk,
    })
}

/// Setup DepositCircuit
pub fn setup_deposit(
    rng: &mut StdRng,
    config: Arc<ark_crypto_primitives::sponge::poseidon::PoseidonConfig<Fr>>,
) -> Result<CircuitKeyPair, SetupError> {
    let circuit = DepositCircuit::empty(config);
    let (pk, vk) = Groth16::<Bn254>::circuit_specific_setup(circuit, rng)
        .map_err(|e| SetupError::CircuitSetup(e.to_string()))?;

    Ok(CircuitKeyPair {
        proving_key: pk,
        verifying_key: vk,
    })
}

/// Setup TransferCircuit
pub fn setup_transfer(
    rng: &mut StdRng,
    config: Arc<ark_crypto_primitives::sponge::poseidon::PoseidonConfig<Fr>>,
) -> Result<CircuitKeyPair, SetupError> {
    let circuit = TransferCircuit::empty(config);
    let (pk, vk) = Groth16::<Bn254>::circuit_specific_setup(circuit, rng)
        .map_err(|e| SetupError::CircuitSetup(e.to_string()))?;

    Ok(CircuitKeyPair {
        proving_key: pk,
        verifying_key: vk,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_setup_item_exists() {
        let mut rng = StdRng::seed_from_u64(42);
        let config = Arc::new(poseidon_config::<Fr>());
        let keys = setup_item_exists(&mut rng, config).unwrap();

        // Verify keys can be serialized and deserialized
        let pk_bytes = keys.serialize_pk().unwrap();
        let vk_bytes = keys.serialize_vk().unwrap();

        let _pk = CircuitKeyPair::deserialize_pk(&pk_bytes).unwrap();
        let _vk = CircuitKeyPair::deserialize_vk(&vk_bytes).unwrap();
    }
}
