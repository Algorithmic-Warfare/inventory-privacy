//! State Transition Circuit for SMT-based inventory operations.
//!
//! This circuit proves a valid deposit or withdrawal with capacity checking.
//! It combines the functionality of the old deposit, withdraw, and capacity circuits.
//!
//! Public inputs:
//! - signal_hash: Poseidon hash binding all operation parameters
//! - nonce: Replay protection (verified on-chain against inventory.nonce)
//! - inventory_id: Cross-inventory protection (verified on-chain)
//! - registry_root: Volume registry commitment (verified against VolumeRegistry)
//!
//! Witnesses:
//! - Old inventory state (root, volume, blinding)
//! - New inventory state (root, volume, blinding)
//! - Item details (id, old_quantity, new_quantity)
//! - Merkle proof for the item
//! - Registry proof for item volume lookup
//! - Operation parameters (amount, op_type, max_capacity)

use ark_crypto_primitives::sponge::poseidon::PoseidonConfig;
use ark_crypto_primitives::sponge::Absorb;
use ark_ff::PrimeField;
use ark_r1cs_std::fields::fp::FpVar;
use ark_r1cs_std::prelude::*;
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};
use std::marker::PhantomData;
use std::sync::Arc;

use crate::range_check::{enforce_geq, enforce_u64_range};
use crate::signal::{compute_signal_hash, OpType};
use crate::smt::{verify_and_update, MerkleProof, MerkleProofVar};
use crate::smt_commitment::create_smt_commitment_var;

/// State Transition Circuit.
///
/// Proves a valid deposit or withdrawal operation with capacity checking.
#[derive(Clone)]
pub struct StateTransitionCircuit<F: PrimeField + Absorb> {
    // Public inputs
    /// Expected signal hash (binds all parameters)
    pub signal_hash: Option<F>,
    /// Nonce for replay protection (verified on-chain)
    pub nonce: Option<u64>,
    /// Inventory ID for cross-inventory protection (verified on-chain)
    pub inventory_id: Option<F>,

    // Old state witnesses
    /// Old inventory SMT root
    pub old_inventory_root: Option<F>,
    /// Old total volume
    pub old_volume: Option<u64>,
    /// Old blinding factor
    pub old_blinding: Option<F>,

    // New state witnesses
    /// New inventory SMT root
    pub new_inventory_root: Option<F>,
    /// New total volume
    pub new_volume: Option<u64>,
    /// New blinding factor
    pub new_blinding: Option<F>,

    // Item operation witnesses
    /// Item ID being operated on
    pub item_id: Option<u64>,
    /// Old quantity of the item
    pub old_quantity: Option<u64>,
    /// New quantity of the item
    pub new_quantity: Option<u64>,
    /// Amount being deposited/withdrawn
    pub amount: Option<u64>,
    /// Operation type (deposit/withdraw)
    pub op_type: Option<OpType>,

    // Merkle proof
    /// Proof for item in inventory SMT
    pub inventory_proof: Option<MerkleProof<F>>,

    // Registry witnesses (for volume lookup)
    /// Volume per unit of this item type
    pub item_volume: Option<u64>,
    /// Registry root (commitment to volume table)
    pub registry_root: Option<F>,

    // Capacity
    /// Maximum allowed capacity
    pub max_capacity: Option<u64>,

    /// Poseidon configuration
    pub poseidon_config: Arc<PoseidonConfig<F>>,

    _marker: PhantomData<F>,
}

impl<F: PrimeField + Absorb> StateTransitionCircuit<F> {
    /// Create a new empty circuit for setup.
    /// Uses dummy values that produce valid constraint structure.
    pub fn empty(poseidon_config: Arc<PoseidonConfig<F>>) -> Self {
        use crate::smt::DEFAULT_DEPTH;

        // Create dummy proof with correct depth
        let dummy_proof = MerkleProof::new(
            vec![F::zero(); DEFAULT_DEPTH],
            vec![false; DEFAULT_DEPTH],
        );

        Self {
            signal_hash: Some(F::zero()),
            nonce: Some(0),
            inventory_id: Some(F::zero()),
            old_inventory_root: Some(F::zero()),
            old_volume: Some(0),
            old_blinding: Some(F::zero()),
            new_inventory_root: Some(F::zero()),
            new_volume: Some(0),
            new_blinding: Some(F::zero()),
            item_id: Some(0),
            old_quantity: Some(0),
            new_quantity: Some(0),
            amount: Some(0),
            op_type: Some(OpType::Deposit),
            inventory_proof: Some(dummy_proof),
            item_volume: Some(0),
            registry_root: Some(F::zero()),
            max_capacity: Some(0),
            poseidon_config,
            _marker: PhantomData,
        }
    }

    /// Create a new circuit with all witnesses.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        old_inventory_root: F,
        old_volume: u64,
        old_blinding: F,
        new_inventory_root: F,
        new_volume: u64,
        new_blinding: F,
        item_id: u64,
        old_quantity: u64,
        new_quantity: u64,
        amount: u64,
        op_type: OpType,
        inventory_proof: MerkleProof<F>,
        item_volume: u64,
        registry_root: F,
        max_capacity: u64,
        nonce: u64,
        inventory_id: F,
        poseidon_config: Arc<PoseidonConfig<F>>,
    ) -> Self {
        // Compute commitments
        let old_commitment = crate::smt_commitment::create_smt_commitment(
            old_inventory_root,
            old_volume,
            old_blinding,
            &poseidon_config,
        );
        let new_commitment = crate::smt_commitment::create_smt_commitment(
            new_inventory_root,
            new_volume,
            new_blinding,
            &poseidon_config,
        );

        // Compute signal hash (includes nonce and inventory_id for replay/cross-inventory protection)
        let signal_hash = compute_signal_hash(
            old_commitment,
            new_commitment,
            registry_root,
            max_capacity,
            item_id,
            amount,
            op_type,
            nonce,
            inventory_id,
            &poseidon_config,
        );

        Self {
            signal_hash: Some(signal_hash),
            nonce: Some(nonce),
            inventory_id: Some(inventory_id),
            old_inventory_root: Some(old_inventory_root),
            old_volume: Some(old_volume),
            old_blinding: Some(old_blinding),
            new_inventory_root: Some(new_inventory_root),
            new_volume: Some(new_volume),
            new_blinding: Some(new_blinding),
            item_id: Some(item_id),
            old_quantity: Some(old_quantity),
            new_quantity: Some(new_quantity),
            amount: Some(amount),
            op_type: Some(op_type),
            inventory_proof: Some(inventory_proof),
            item_volume: Some(item_volume),
            registry_root: Some(registry_root),
            max_capacity: Some(max_capacity),
            poseidon_config,
            _marker: PhantomData,
        }
    }
}

impl<F: PrimeField + Absorb> ConstraintSynthesizer<F> for StateTransitionCircuit<F> {
    fn generate_constraints(self, cs: ConstraintSystemRef<F>) -> Result<(), SynthesisError> {
        // === Allocate public inputs ===
        // Order matters: signal_hash, nonce, inventory_id, registry_root
        let signal_hash_var = FpVar::new_input(cs.clone(), || {
            self.signal_hash.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let nonce_var = FpVar::new_input(cs.clone(), || {
            self.nonce
                .map(F::from)
                .ok_or(SynthesisError::AssignmentMissing)
        })?;
        let inventory_id_var = FpVar::new_input(cs.clone(), || {
            self.inventory_id.ok_or(SynthesisError::AssignmentMissing)
        })?;

        // === Allocate old state witnesses ===
        let old_root_var = FpVar::new_witness(cs.clone(), || {
            self.old_inventory_root.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let old_volume_var = FpVar::new_witness(cs.clone(), || {
            self.old_volume
                .map(F::from)
                .ok_or(SynthesisError::AssignmentMissing)
        })?;
        let old_blinding_var = FpVar::new_witness(cs.clone(), || {
            self.old_blinding.ok_or(SynthesisError::AssignmentMissing)
        })?;

        // === Allocate new state witnesses ===
        let new_root_var = FpVar::new_witness(cs.clone(), || {
            self.new_inventory_root.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let new_volume_var = FpVar::new_witness(cs.clone(), || {
            self.new_volume
                .map(F::from)
                .ok_or(SynthesisError::AssignmentMissing)
        })?;
        let new_blinding_var = FpVar::new_witness(cs.clone(), || {
            self.new_blinding.ok_or(SynthesisError::AssignmentMissing)
        })?;

        // === Allocate item operation witnesses ===
        let item_id_var = FpVar::new_witness(cs.clone(), || {
            self.item_id
                .map(F::from)
                .ok_or(SynthesisError::AssignmentMissing)
        })?;
        let old_qty_var = FpVar::new_witness(cs.clone(), || {
            self.old_quantity
                .map(F::from)
                .ok_or(SynthesisError::AssignmentMissing)
        })?;
        let new_qty_var = FpVar::new_witness(cs.clone(), || {
            self.new_quantity
                .map(F::from)
                .ok_or(SynthesisError::AssignmentMissing)
        })?;
        let amount_var = FpVar::new_witness(cs.clone(), || {
            self.amount
                .map(F::from)
                .ok_or(SynthesisError::AssignmentMissing)
        })?;
        let op_type_var = FpVar::new_witness(cs.clone(), || {
            self.op_type
                .map(|op| op.to_field::<F>())
                .ok_or(SynthesisError::AssignmentMissing)
        })?;

        // === Allocate Merkle proof ===
        let proof = self.inventory_proof.as_ref();
        let inventory_proof_var = MerkleProofVar::new_witness(cs.clone(), proof.unwrap())?;

        // === Allocate registry public input ===
        // registry_root is a public input so it can be verified on-chain against VolumeRegistry
        let registry_root_var = FpVar::new_input(cs.clone(), || {
            self.registry_root.ok_or(SynthesisError::AssignmentMissing)
        })?;

        // === Allocate registry witnesses ===
        let item_volume_var = FpVar::new_witness(cs.clone(), || {
            self.item_volume
                .map(F::from)
                .ok_or(SynthesisError::AssignmentMissing)
        })?;
        let max_capacity_var = FpVar::new_witness(cs.clone(), || {
            self.max_capacity
                .map(F::from)
                .ok_or(SynthesisError::AssignmentMissing)
        })?;

        // === Constraint 1: Verify and update inventory SMT ===
        // This verifies the old state and computes the new root
        let computed_new_root = verify_and_update(
            cs.clone(),
            &old_root_var,
            &item_id_var,
            &old_qty_var,
            &new_qty_var,
            &inventory_proof_var,
            &self.poseidon_config,
        )?;

        // Enforce computed new root matches claimed new root
        computed_new_root.enforce_equal(&new_root_var)?;

        // === Constraint 2: Verify quantity change matches operation ===
        // For deposit: new_qty = old_qty + amount
        // For withdraw: new_qty = old_qty - amount
        let zero = FpVar::zero();
        let one = FpVar::one();
        let is_deposit = op_type_var.is_eq(&zero)?;

        // Compute expected new quantity based on operation type
        let qty_plus_amount = &old_qty_var + &amount_var;
        let qty_minus_amount = &old_qty_var - &amount_var;
        let expected_new_qty = is_deposit.select(&qty_plus_amount, &qty_minus_amount)?;

        new_qty_var.enforce_equal(&expected_new_qty)?;

        // === Constraint 3: Range check on new quantity ===
        // Prevents underflow attacks where withdraw > current quantity
        // If qty_minus_amount wrapped around (negative), it won't fit in 64 bits
        enforce_u64_range(cs.clone(), &new_qty_var)?;

        // === Constraint 4: Verify volume change ===
        // volume_delta = item_volume * amount
        let volume_delta = &item_volume_var * &amount_var;

        // For deposit: new_volume = old_volume + volume_delta
        // For withdraw: new_volume = old_volume - volume_delta
        let vol_plus_delta = &old_volume_var + &volume_delta;
        let vol_minus_delta = &old_volume_var - &volume_delta;
        let expected_new_volume = is_deposit.select(&vol_plus_delta, &vol_minus_delta)?;

        new_volume_var.enforce_equal(&expected_new_volume)?;

        // === Constraint 5: Range check on new volume ===
        // Prevents underflow attacks on volume
        enforce_u64_range(cs.clone(), &new_volume_var)?;

        // === Constraint 6: Capacity check ===
        // new_volume <= max_capacity
        // enforce_geq checks that (max_capacity - new_volume) fits in 64 bits
        enforce_geq(cs.clone(), &max_capacity_var, &new_volume_var)?;

        // === Constraint 7: Compute commitments ===
        let old_commitment_var = create_smt_commitment_var(
            cs.clone(),
            &old_root_var,
            &old_volume_var,
            &old_blinding_var,
            &self.poseidon_config,
        )?;

        let new_commitment_var = create_smt_commitment_var(
            cs.clone(),
            &new_root_var,
            &new_volume_var,
            &new_blinding_var,
            &self.poseidon_config,
        )?;

        // === Constraint 8: Compute and verify signal hash ===
        // Signal hash now includes nonce and inventory_id for replay/cross-inventory protection
        let computed_signal = crate::signal::compute_signal_hash_var(
            cs.clone(),
            &old_commitment_var,
            &new_commitment_var,
            &registry_root_var,
            &max_capacity_var,
            &item_id_var,
            &amount_var,
            &op_type_var,
            &nonce_var,
            &inventory_id_var,
            &self.poseidon_config,
        )?;

        computed_signal.enforce_equal(&signal_hash_var)?;

        // === Constraint 9: Ensure op_type is valid (0 or 1) ===
        let is_withdraw = op_type_var.is_eq(&one)?;
        let is_valid_op = is_deposit.or(&is_withdraw)?;
        is_valid_op.enforce_equal(&Boolean::TRUE)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commitment::poseidon_config;
    use crate::smt::{SparseMerkleTree, DEFAULT_DEPTH};
    use ark_bn254::Fr;
    use ark_relations::r1cs::ConstraintSystem;

    fn setup() -> Arc<PoseidonConfig<Fr>> {
        Arc::new(poseidon_config())
    }

    #[test]
    fn test_state_transition_deposit() {
        let config = setup();

        // Create initial inventory with 1 item
        let mut tree = SparseMerkleTree::<Fr>::from_items(
            &[(1, 100)],
            DEFAULT_DEPTH,
            config.clone(),
        );
        let old_root = tree.root();
        let proof = tree.get_proof(1);

        // Deposit 50 more
        tree.update(1, 150);
        let new_root = tree.root();

        // Create circuit
        let old_blinding = Fr::from(12345u64);
        let new_blinding = Fr::from(67890u64);
        let item_volume = 10u64;
        let old_volume = 100 * item_volume; // 100 items * 10 volume each
        let new_volume = 150 * item_volume; // 150 items * 10 volume each
        let registry_root = Fr::from(99999u64);
        let max_capacity = 10000u64;

        let nonce = 0u64;
        let inventory_id = Fr::from(12345678u64);

        let circuit = StateTransitionCircuit::new(
            old_root,
            old_volume,
            old_blinding,
            new_root,
            new_volume,
            new_blinding,
            1,   // item_id
            100, // old_quantity
            150, // new_quantity
            50,  // amount
            OpType::Deposit,
            proof,
            item_volume,
            registry_root,
            max_capacity,
            nonce,
            inventory_id,
            config.clone(),
        );

        let cs = ConstraintSystem::<Fr>::new_ref();
        circuit.generate_constraints(cs.clone()).unwrap();

        assert!(cs.is_satisfied().unwrap());
        println!("StateTransition (deposit) constraints: {}", cs.num_constraints());
    }

    #[test]
    fn test_state_transition_withdraw() {
        let config = setup();

        // Create initial inventory with 1 item
        let mut tree = SparseMerkleTree::<Fr>::from_items(
            &[(1, 100)],
            DEFAULT_DEPTH,
            config.clone(),
        );
        let old_root = tree.root();
        let proof = tree.get_proof(1);

        // Withdraw 30
        tree.update(1, 70);
        let new_root = tree.root();

        // Create circuit
        let old_blinding = Fr::from(12345u64);
        let new_blinding = Fr::from(67890u64);
        let item_volume = 10u64;
        let old_volume = 100 * item_volume;
        let new_volume = 70 * item_volume;
        let registry_root = Fr::from(99999u64);
        let max_capacity = 10000u64;
        let nonce = 5u64;
        let inventory_id = Fr::from(12345678u64);

        let circuit = StateTransitionCircuit::new(
            old_root,
            old_volume,
            old_blinding,
            new_root,
            new_volume,
            new_blinding,
            1,   // item_id
            100, // old_quantity
            70,  // new_quantity
            30,  // amount
            OpType::Withdraw,
            proof,
            item_volume,
            registry_root,
            max_capacity,
            nonce,
            inventory_id,
            config.clone(),
        );

        let cs = ConstraintSystem::<Fr>::new_ref();
        circuit.generate_constraints(cs.clone()).unwrap();

        assert!(cs.is_satisfied().unwrap());
        println!("StateTransition (withdraw) constraints: {}", cs.num_constraints());
    }

    #[test]
    fn test_state_transition_new_item() {
        let config = setup();

        // Create empty inventory
        let mut tree = SparseMerkleTree::<Fr>::new(DEFAULT_DEPTH, config.clone());
        let old_root = tree.root();
        let proof = tree.get_proof(42); // Proof for empty slot

        // Add new item
        tree.update(42, 100);
        let new_root = tree.root();

        let old_blinding = Fr::from(12345u64);
        let new_blinding = Fr::from(67890u64);
        let item_volume = 5u64;
        let old_volume = 0u64;
        let new_volume = 100 * item_volume;
        let registry_root = Fr::from(99999u64);
        let max_capacity = 10000u64;
        let nonce = 0u64;
        let inventory_id = Fr::from(12345678u64);

        let circuit = StateTransitionCircuit::new(
            old_root,
            old_volume,
            old_blinding,
            new_root,
            new_volume,
            new_blinding,
            42,  // item_id
            0,   // old_quantity (empty slot)
            100, // new_quantity
            100, // amount
            OpType::Deposit,
            proof,
            item_volume,
            registry_root,
            max_capacity,
            nonce,
            inventory_id,
            config.clone(),
        );

        let cs = ConstraintSystem::<Fr>::new_ref();
        circuit.generate_constraints(cs.clone()).unwrap();

        assert!(cs.is_satisfied().unwrap());
        println!("StateTransition (new item) constraints: {}", cs.num_constraints());
    }

    #[test]
    fn test_state_transition_wrong_amount() {
        let config = setup();

        let mut tree = SparseMerkleTree::<Fr>::from_items(
            &[(1, 100)],
            DEFAULT_DEPTH,
            config.clone(),
        );
        let old_root = tree.root();
        let proof = tree.get_proof(1);

        tree.update(1, 150);
        let new_root = tree.root();

        let old_blinding = Fr::from(12345u64);
        let new_blinding = Fr::from(67890u64);
        let item_volume = 10u64;
        let old_volume = 100 * item_volume;
        let new_volume = 150 * item_volume;
        let registry_root = Fr::from(99999u64);
        let max_capacity = 10000u64;
        let nonce = 0u64;
        let inventory_id = Fr::from(12345678u64);

        // Try to claim we deposited 60 instead of 50
        let circuit = StateTransitionCircuit::new(
            old_root,
            old_volume,
            old_blinding,
            new_root,
            new_volume,
            new_blinding,
            1,
            100,
            150,
            60, // WRONG amount
            OpType::Deposit,
            proof,
            item_volume,
            registry_root,
            max_capacity,
            nonce,
            inventory_id,
            config.clone(),
        );

        let cs = ConstraintSystem::<Fr>::new_ref();
        circuit.generate_constraints(cs.clone()).unwrap();

        // Should fail because 100 + 60 != 150
        assert!(!cs.is_satisfied().unwrap());
    }

    #[test]
    fn test_state_transition_wrong_volume() {
        let config = setup();

        let mut tree = SparseMerkleTree::<Fr>::from_items(
            &[(1, 100)],
            DEFAULT_DEPTH,
            config.clone(),
        );
        let old_root = tree.root();
        let proof = tree.get_proof(1);

        tree.update(1, 150);
        let new_root = tree.root();

        let old_blinding = Fr::from(12345u64);
        let new_blinding = Fr::from(67890u64);
        let item_volume = 10u64;
        let old_volume = 100 * item_volume;
        let registry_root = Fr::from(99999u64);
        let max_capacity = 10000u64;
        let nonce = 0u64;
        let inventory_id = Fr::from(12345678u64);

        // Claim wrong new volume
        let circuit = StateTransitionCircuit::new(
            old_root,
            old_volume,
            old_blinding,
            new_root,
            1600, // WRONG new volume (should be 1500)
            new_blinding,
            1,
            100,
            150,
            50,
            OpType::Deposit,
            proof,
            item_volume,
            registry_root,
            max_capacity,
            nonce,
            inventory_id,
            config.clone(),
        );

        let cs = ConstraintSystem::<Fr>::new_ref();
        circuit.generate_constraints(cs.clone()).unwrap();

        // Should fail because volume doesn't match
        assert!(!cs.is_satisfied().unwrap());
    }

    #[test]
    fn test_underflow_attack_blocked() {
        // This test verifies that the range check prevents underflow attacks
        let config = setup();

        let mut tree = SparseMerkleTree::<Fr>::from_items(
            &[(1, 50)],  // Only have 50 items
            DEFAULT_DEPTH,
            config.clone(),
        );
        let old_root = tree.root();
        let proof = tree.get_proof(1);

        // Attacker tries to withdraw 100 when only 50 exist
        // Without range checks, 50 - 100 would wrap to a huge number
        tree.update(1, 0); // Pretend we end up with 0 (invalid)
        let new_root = tree.root();

        let old_blinding = Fr::from(12345u64);
        let new_blinding = Fr::from(67890u64);
        let item_volume = 10u64;
        let old_volume = 50 * item_volume;
        let registry_root = Fr::from(99999u64);
        let max_capacity = 10000u64;
        let nonce = 0u64;
        let inventory_id = Fr::from(12345678u64);

        // The new_quantity would be 50 - 100 = -50, which wraps in field arithmetic
        // But our range check should catch this
        let wrapped_qty = Fr::from(50u64) - Fr::from(100u64); // This wraps!

        // We can't even create a valid circuit because the signal hash would be wrong
        // But let's verify the range check works by creating an empty circuit
        // and manually testing the constraint
        let cs = ConstraintSystem::<Fr>::new_ref();

        // Allocate the wrapped value and try to range check it
        let wrapped_var = FpVar::new_witness(cs.clone(), || Ok(wrapped_qty)).unwrap();

        // This should fail - the wrapped value doesn't fit in 64 bits
        crate::range_check::enforce_u64_range(cs.clone(), &wrapped_var).unwrap();

        assert!(!cs.is_satisfied().unwrap(), "Range check should reject wrapped negative value");
    }
}
