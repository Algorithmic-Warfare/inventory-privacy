//! Trusted setup utilities for generating proving and verifying keys.

use ark_bn254::Bn254;
use ark_groth16::{Groth16, ProvingKey, VerifyingKey};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use ark_snark::SNARK;
use ark_std::rand::rngs::StdRng;
use thiserror::Error;

use inventory_circuits::{
    CapacitySMTCircuit, ItemExistsSMTCircuit, StateTransitionCircuit,
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

/// All circuit keys for SMT-based circuits
pub struct CircuitKeys {
    /// StateTransition circuit (deposit/withdraw with capacity)
    pub state_transition: CircuitKeyPair,
    /// ItemExists circuit (prove ownership of items)
    pub item_exists: CircuitKeyPair,
    /// Capacity circuit (prove volume compliance)
    pub capacity: CircuitKeyPair,
}

impl CircuitKeys {
    /// Save all keys to a directory
    pub fn save_to_directory(&self, dir: &std::path::Path) -> Result<(), SetupError> {
        std::fs::create_dir_all(dir)?;

        std::fs::write(
            dir.join("state_transition.pk"),
            self.state_transition.serialize_pk()?,
        )?;
        std::fs::write(
            dir.join("state_transition.vk"),
            self.state_transition.serialize_vk()?,
        )?;

        std::fs::write(dir.join("item_exists.pk"), self.item_exists.serialize_pk()?)?;
        std::fs::write(dir.join("item_exists.vk"), self.item_exists.serialize_vk()?)?;

        std::fs::write(dir.join("capacity.pk"), self.capacity.serialize_pk()?)?;
        std::fs::write(dir.join("capacity.vk"), self.capacity.serialize_vk()?)?;

        Ok(())
    }

    /// Load all keys from a directory
    pub fn load_from_directory(dir: &std::path::Path) -> Result<Self, SetupError> {
        let state_transition = CircuitKeyPair {
            proving_key: CircuitKeyPair::deserialize_pk(&std::fs::read(
                dir.join("state_transition.pk"),
            )?)?,
            verifying_key: CircuitKeyPair::deserialize_vk(&std::fs::read(
                dir.join("state_transition.vk"),
            )?)?,
        };

        let item_exists = CircuitKeyPair {
            proving_key: CircuitKeyPair::deserialize_pk(&std::fs::read(
                dir.join("item_exists.pk"),
            )?)?,
            verifying_key: CircuitKeyPair::deserialize_vk(&std::fs::read(
                dir.join("item_exists.vk"),
            )?)?,
        };

        let capacity = CircuitKeyPair {
            proving_key: CircuitKeyPair::deserialize_pk(&std::fs::read(dir.join("capacity.pk"))?)?,
            verifying_key: CircuitKeyPair::deserialize_vk(&std::fs::read(
                dir.join("capacity.vk"),
            )?)?,
        };

        Ok(Self {
            state_transition,
            item_exists,
            capacity,
        })
    }
}

/// Run trusted setup for all SMT circuits
pub fn setup_all_circuits() -> Result<CircuitKeys, SetupError> {
    // Use a fixed seed for reproducible setup (in production, use secure randomness)
    let mut rng = ark_std::rand::SeedableRng::seed_from_u64(42);

    println!("Setting up StateTransitionCircuit...");
    let state_transition = setup_state_transition(&mut rng)?;

    println!("Setting up ItemExistsSMTCircuit...");
    let item_exists = setup_item_exists(&mut rng)?;

    println!("Setting up CapacitySMTCircuit...");
    let capacity = setup_capacity(&mut rng)?;

    Ok(CircuitKeys {
        state_transition,
        item_exists,
        capacity,
    })
}

/// Setup StateTransitionCircuit
pub fn setup_state_transition(
    rng: &mut StdRng,
) -> Result<CircuitKeyPair, SetupError> {
    let circuit = StateTransitionCircuit::empty();
    let (pk, vk) = Groth16::<Bn254>::circuit_specific_setup(circuit, rng)
        .map_err(|e| SetupError::CircuitSetup(e.to_string()))?;

    Ok(CircuitKeyPair {
        proving_key: pk,
        verifying_key: vk,
    })
}

/// Setup ItemExistsSMTCircuit
pub fn setup_item_exists(
    rng: &mut StdRng,
) -> Result<CircuitKeyPair, SetupError> {
    let circuit = ItemExistsSMTCircuit::empty();
    let (pk, vk) = Groth16::<Bn254>::circuit_specific_setup(circuit, rng)
        .map_err(|e| SetupError::CircuitSetup(e.to_string()))?;

    Ok(CircuitKeyPair {
        proving_key: pk,
        verifying_key: vk,
    })
}

/// Setup CapacitySMTCircuit
pub fn setup_capacity(
    rng: &mut StdRng,
) -> Result<CircuitKeyPair, SetupError> {
    let circuit = CapacitySMTCircuit::empty();
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
    use ark_std::rand::SeedableRng;

    #[test]
    fn test_setup_state_transition() {
        let mut rng = StdRng::seed_from_u64(42);
        let keys = setup_state_transition(&mut rng).unwrap();

        // Verify keys can be serialized and deserialized
        let pk_bytes = keys.serialize_pk().unwrap();
        let vk_bytes = keys.serialize_vk().unwrap();

        let _pk = CircuitKeyPair::deserialize_pk(&pk_bytes).unwrap();
        let _vk = CircuitKeyPair::deserialize_vk(&vk_bytes).unwrap();
    }

    #[test]
    fn test_setup_item_exists() {
        let mut rng = StdRng::seed_from_u64(42);
        let keys = setup_item_exists(&mut rng).unwrap();

        let pk_bytes = keys.serialize_pk().unwrap();
        let vk_bytes = keys.serialize_vk().unwrap();

        let _pk = CircuitKeyPair::deserialize_pk(&pk_bytes).unwrap();
        let _vk = CircuitKeyPair::deserialize_vk(&vk_bytes).unwrap();
    }

    #[test]
    fn test_setup_capacity() {
        let mut rng = StdRng::seed_from_u64(42);
        let keys = setup_capacity(&mut rng).unwrap();

        let pk_bytes = keys.serialize_pk().unwrap();
        let vk_bytes = keys.serialize_vk().unwrap();

        let _pk = CircuitKeyPair::deserialize_pk(&pk_bytes).unwrap();
        let _vk = CircuitKeyPair::deserialize_vk(&vk_bytes).unwrap();
    }
}
