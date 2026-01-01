//! Proof generation for SMT-based inventory circuits.

use ark_bn254::{Bn254, Fr};
use ark_groth16::{Groth16, Proof, ProvingKey};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use ark_snark::SNARK;
use ark_std::rand::{rngs::StdRng, SeedableRng};
use thiserror::Error;

use inventory_circuits::{
    signal::OpType,
    smt::{MerkleProof, SparseMerkleTree, DEFAULT_DEPTH},
    smt_commitment::create_smt_commitment,
    CapacitySMTCircuit, ItemExistsSMTCircuit, StateTransitionCircuit,
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

/// A proof with its public inputs (signal hash)
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

/// Client-side inventory state using SMT
#[derive(Clone)]
pub struct InventoryState {
    /// Sparse Merkle Tree storing items
    pub tree: SparseMerkleTree,
    /// Current total volume
    pub current_volume: u64,
    /// Blinding factor for commitment
    pub blinding: Fr,
}

impl InventoryState {
    /// Create a new empty inventory state
    pub fn new(blinding: Fr) -> Self {
        Self {
            tree: SparseMerkleTree::new(DEFAULT_DEPTH),
            current_volume: 0,
            blinding,
        }
    }

    /// Create inventory state from items
    pub fn from_items(items: &[(u64, u64)], blinding: Fr) -> Self {
        let tree = SparseMerkleTree::from_items(items, DEFAULT_DEPTH);
        Self {
            tree,
            current_volume: 0, // Volume must be set separately
            blinding,
        }
    }

    /// Get the inventory SMT root
    pub fn root(&self) -> Fr {
        self.tree.root()
    }

    /// Get quantity of an item
    pub fn get_quantity(&self, item_id: u64) -> u64 {
        self.tree.get(item_id)
    }

    /// Get Merkle proof for an item
    pub fn get_proof(&self, item_id: u64) -> MerkleProof<Fr> {
        self.tree.get_proof(item_id)
    }

    /// Compute the commitment for this inventory state
    pub fn commitment(&self) -> Fr {
        create_smt_commitment(
            self.tree.root(),
            self.current_volume,
            self.blinding,
        )
    }

    /// Deposit items (returns updated state and proof)
    pub fn deposit(
        &self,
        item_id: u64,
        amount: u64,
        item_volume: u64,
        new_blinding: Fr,
    ) -> Result<(InventoryState, MerkleProof<Fr>), ProveError> {
        let old_qty = self.get_quantity(item_id);
        let new_qty = old_qty.checked_add(amount)
            .ok_or_else(|| ProveError::InvalidState("Quantity overflow".into()))?;

        // Get proof before update
        let proof = self.get_proof(item_id);

        // Update tree
        let mut new_tree = self.tree.clone();
        new_tree.update(item_id, new_qty);

        // Update volume
        let volume_delta = amount * item_volume;
        let new_volume = self.current_volume.checked_add(volume_delta)
            .ok_or_else(|| ProveError::InvalidState("Volume overflow".into()))?;

        Ok((
            InventoryState {
                tree: new_tree,
                current_volume: new_volume,
                blinding: new_blinding,
            },
            proof,
        ))
    }

    /// Withdraw items (returns updated state and proof)
    pub fn withdraw(
        &self,
        item_id: u64,
        amount: u64,
        item_volume: u64,
        new_blinding: Fr,
    ) -> Result<(InventoryState, MerkleProof<Fr>), ProveError> {
        let old_qty = self.get_quantity(item_id);
        if old_qty < amount {
            return Err(ProveError::InvalidState(format!(
                "Insufficient quantity: have {}, need {}",
                old_qty, amount
            )));
        }
        let new_qty = old_qty - amount;

        // Get proof before update
        let proof = self.get_proof(item_id);

        // Update tree (setting to 0 removes the item)
        let mut new_tree = self.tree.clone();
        new_tree.update(item_id, new_qty);

        // Update volume
        let volume_delta = amount * item_volume;
        let new_volume = self.current_volume.saturating_sub(volume_delta);

        Ok((
            InventoryState {
                tree: new_tree,
                current_volume: new_volume,
                blinding: new_blinding,
            },
            proof,
        ))
    }
}

/// Result of a state transition proof
pub struct StateTransitionResult {
    pub proof: ProofWithInputs,
    pub new_state: InventoryState,
    pub new_commitment: Fr,
    /// Nonce used in this proof (for on-chain verification)
    pub nonce: u64,
    /// Inventory ID used in this proof (for on-chain verification)
    pub inventory_id: Fr,
    /// Registry root used in this proof (for on-chain verification)
    pub registry_root: Fr,
}

/// Generate proof for StateTransitionCircuit (deposit or withdraw)
///
/// # Arguments
/// * `pk` - Proving key for StateTransitionCircuit
/// * `old_state` - Current inventory state
/// * `new_blinding` - New blinding factor for the updated commitment
/// * `item_id` - Item being deposited/withdrawn
/// * `amount` - Quantity being deposited/withdrawn
/// * `item_volume` - Volume per unit of this item type
/// * `registry_root` - VolumeRegistry hash (must match on-chain)
/// * `max_capacity` - Maximum allowed volume (0 = unlimited)
/// * `nonce` - Current inventory nonce (must match on-chain, for replay protection)
/// * `inventory_id` - Inventory object ID as field element (must match on-chain)
/// * `op_type` - Deposit or Withdraw
#[allow(clippy::too_many_arguments)]
pub fn prove_state_transition(
    pk: &ProvingKey<Bn254>,
    old_state: &InventoryState,
    new_blinding: Fr,
    item_id: u64,
    amount: u64,
    item_volume: u64,
    registry_root: Fr,
    max_capacity: u64,
    nonce: u64,
    inventory_id: Fr,
    op_type: OpType,
) -> Result<StateTransitionResult, ProveError> {
    // Get old quantities and proof
    let old_quantity = old_state.get_quantity(item_id);
    let inventory_proof = old_state.get_proof(item_id);

    // Compute new state
    let (new_quantity, new_volume) = match op_type {
        OpType::Deposit => {
            let new_qty = old_quantity.checked_add(amount)
                .ok_or_else(|| ProveError::InvalidState("Quantity overflow".into()))?;
            let volume_delta = amount * item_volume;
            let new_vol = old_state.current_volume.checked_add(volume_delta)
                .ok_or_else(|| ProveError::InvalidState("Volume overflow".into()))?;
            // max_capacity of 0 means unlimited
            if max_capacity > 0 && new_vol > max_capacity {
                return Err(ProveError::InvalidState(format!(
                    "Capacity exceeded: {} > {}",
                    new_vol, max_capacity
                )));
            }
            (new_qty, new_vol)
        }
        OpType::Withdraw => {
            if old_quantity < amount {
                return Err(ProveError::InvalidState(format!(
                    "Insufficient quantity: have {}, need {}",
                    old_quantity, amount
                )));
            }
            let new_qty = old_quantity - amount;
            let volume_delta = amount * item_volume;
            let new_vol = old_state.current_volume.saturating_sub(volume_delta);
            (new_qty, new_vol)
        }
    };

    // Create new tree state
    let mut new_tree = old_state.tree.clone();
    new_tree.update(item_id, new_quantity);

    let new_state = InventoryState {
        tree: new_tree,
        current_volume: new_volume,
        blinding: new_blinding,
    };

    let new_commitment = new_state.commitment();

    // Create circuit with all security parameters
    let circuit = StateTransitionCircuit::new(
        old_state.tree.root(),
        old_state.current_volume,
        old_state.blinding,
        new_state.tree.root(),
        new_volume,
        new_blinding,
        item_id,
        old_quantity,
        new_quantity,
        amount,
        op_type,
        inventory_proof,
        item_volume,
        registry_root,
        max_capacity,
        nonce,
        inventory_id,
    );

    let signal_hash = circuit.signal_hash.unwrap();

    // Generate proof
    let mut rng = StdRng::from_entropy();
    let proof = Groth16::<Bn254>::prove(pk, circuit, &mut rng)
        .map_err(|e| ProveError::ProofGeneration(e.to_string()))?;

    // Return all 4 public inputs for on-chain verification
    // Order: signal_hash, nonce, inventory_id, registry_root
    Ok(StateTransitionResult {
        proof: ProofWithInputs {
            proof,
            public_inputs: vec![signal_hash, Fr::from(nonce), inventory_id, registry_root],
        },
        new_state,
        new_commitment,
        nonce,
        inventory_id,
        registry_root,
    })
}

/// Generate proof for ItemExistsSMTCircuit
pub fn prove_item_exists(
    pk: &ProvingKey<Bn254>,
    state: &InventoryState,
    item_id: u64,
    min_quantity: u64,
) -> Result<ProofWithInputs, ProveError> {
    // Get actual quantity and proof
    let actual_quantity = state.get_quantity(item_id);
    if actual_quantity < min_quantity {
        return Err(ProveError::InvalidState(format!(
            "Insufficient quantity: have {}, need >= {}",
            actual_quantity, min_quantity
        )));
    }

    let proof = state.get_proof(item_id);

    // Create circuit
    let circuit = ItemExistsSMTCircuit::new(
        state.tree.root(),
        state.current_volume,
        state.blinding,
        item_id,
        actual_quantity,
        min_quantity,
        proof,
    );

    let public_hash = circuit.public_hash.unwrap();

    // Generate proof
    let mut rng = StdRng::from_entropy();
    let zk_proof = Groth16::<Bn254>::prove(pk, circuit, &mut rng)
        .map_err(|e| ProveError::ProofGeneration(e.to_string()))?;

    Ok(ProofWithInputs {
        proof: zk_proof,
        public_inputs: vec![public_hash],
    })
}

/// Generate proof for CapacitySMTCircuit
pub fn prove_capacity(
    pk: &ProvingKey<Bn254>,
    state: &InventoryState,
    max_capacity: u64,
) -> Result<ProofWithInputs, ProveError> {
    // Verify capacity compliance (max_capacity of 0 means unlimited)
    if max_capacity > 0 && state.current_volume > max_capacity {
        return Err(ProveError::InvalidState(format!(
            "Volume exceeds capacity: {} > {}",
            state.current_volume, max_capacity
        )));
    }

    // Create circuit
    let circuit = CapacitySMTCircuit::new(
        state.tree.root(),
        state.current_volume,
        state.blinding,
        max_capacity,
    );

    let public_hash = circuit.public_hash.unwrap();

    // Generate proof
    let mut rng = StdRng::from_entropy();
    let proof = Groth16::<Bn254>::prove(pk, circuit, &mut rng)
        .map_err(|e| ProveError::ProofGeneration(e.to_string()))?;

    Ok(ProofWithInputs {
        proof,
        public_inputs: vec![public_hash],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup::{setup_capacity, setup_item_exists, setup_state_transition};
    use ark_std::rand::SeedableRng;

    #[test]
    fn test_prove_item_exists() {
        let mut rng = StdRng::seed_from_u64(42);
        let keys = setup_item_exists(&mut rng).unwrap();

        // Create inventory with item
        let blinding = Fr::from(12345u64);
        let mut state = InventoryState::new(blinding);
        state.tree.update(42, 100);
        state.current_volume = 500;

        // Prove we have at least 50 of item 42
        let result = prove_item_exists(&keys.proving_key, &state, 42, 50);
        assert!(result.is_ok());

        let proof = result.unwrap();
        assert_eq!(proof.public_inputs.len(), 1); // Single signal hash
    }

    #[test]
    fn test_prove_item_exists_insufficient() {
        let mut rng = StdRng::seed_from_u64(42);
        let keys = setup_item_exists(&mut rng).unwrap();

        let blinding = Fr::from(12345u64);
        let mut state = InventoryState::new(blinding);
        state.tree.update(42, 30);
        state.current_volume = 300;

        // Try to prove we have 50 when we only have 30
        let result = prove_item_exists(&keys.proving_key, &state, 42, 50);
        assert!(result.is_err());
    }

    #[test]
    fn test_prove_capacity() {
        let mut rng = StdRng::seed_from_u64(42);
        let keys = setup_capacity(&mut rng).unwrap();

        let blinding = Fr::from(12345u64);
        let mut state = InventoryState::new(blinding);
        state.tree.update(1, 100);
        state.current_volume = 500; // Below max

        let result = prove_capacity(&keys.proving_key, &state, 1000);
        assert!(result.is_ok());

        let proof = result.unwrap();
        assert_eq!(proof.public_inputs.len(), 1);
    }

    #[test]
    fn test_prove_capacity_exceeded() {
        let mut rng = StdRng::seed_from_u64(42);
        let keys = setup_capacity(&mut rng).unwrap();

        let blinding = Fr::from(12345u64);
        let mut state = InventoryState::new(blinding);
        state.tree.update(1, 100);
        state.current_volume = 1500; // Above max

        let result = prove_capacity(&keys.proving_key, &state, 1000);
        assert!(result.is_err());
    }

    #[test]
    fn test_prove_state_transition_deposit() {
        let mut rng = StdRng::seed_from_u64(42);
        let keys = setup_state_transition(&mut rng).unwrap();

        let blinding = Fr::from(12345u64);
        let new_blinding = Fr::from(67890u64);
        let state = InventoryState::new(blinding);

        // Simple registry root (would normally come from on-chain registry)
        let registry_root = Fr::from(99999u64);
        let nonce = 0u64;
        let inventory_id = Fr::from(12345678u64);

        let result = prove_state_transition(
            &keys.proving_key,
            &state,
            new_blinding,
            1,    // item_id
            5,    // amount
            10,   // item_volume
            registry_root,
            1000, // max_capacity
            nonce,
            inventory_id,
            OpType::Deposit,
        );

        assert!(result.is_ok());
        let res = result.unwrap();
        // Now 4 public inputs: signal_hash, nonce, inventory_id, registry_root
        assert_eq!(res.proof.public_inputs.len(), 4);
        assert_eq!(res.new_state.current_volume, 50); // 5 * 10
        assert_eq!(res.nonce, nonce);
        assert_eq!(res.inventory_id, inventory_id);
        assert_eq!(res.registry_root, registry_root);
    }

    #[test]
    fn test_prove_state_transition_withdraw() {
        let mut rng = StdRng::seed_from_u64(42);
        let keys = setup_state_transition(&mut rng).unwrap();

        let blinding = Fr::from(12345u64);
        let new_blinding = Fr::from(67890u64);

        // Create state with some items
        let mut state = InventoryState::new(blinding);
        state.tree.update(1, 100);
        state.current_volume = 1000; // 100 items * 10 volume each

        // Registry root and security parameters
        let registry_root = Fr::from(99999u64);
        let nonce = 5u64;
        let inventory_id = Fr::from(12345678u64);

        let result = prove_state_transition(
            &keys.proving_key,
            &state,
            new_blinding,
            1,    // item_id
            30,   // amount to withdraw
            10,   // item_volume
            registry_root,
            1000, // max_capacity
            nonce,
            inventory_id,
            OpType::Withdraw,
        );

        assert!(result.is_ok());
        let res = result.unwrap();
        assert_eq!(res.proof.public_inputs.len(), 4);
        assert_eq!(res.new_state.current_volume, 700); // 1000 - 30*10
        assert_eq!(res.new_state.get_quantity(1), 70); // 100 - 30
    }
}
