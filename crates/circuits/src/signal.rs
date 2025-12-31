//! Signal hash for collapsing public inputs.
//!
//! Sui has a limit of 8 public inputs for ZK proofs. The signal hash pattern
//! compresses most inputs into a single hash, with critical context values
//! as separate public inputs for on-chain verification.
//!
//! Public inputs:
//! - signal_hash (binding all parameters)
//! - nonce (for replay protection - verified on-chain)
//! - inventory_id (for cross-inventory protection - verified on-chain)
//! - registry_root (for volume validation - verified against VolumeRegistry)
//!
//! signal_hash = Poseidon(
//!     old_commitment,
//!     new_commitment,
//!     registry_root,
//!     max_capacity,
//!     item_id,
//!     amount,
//!     op_type,
//!     nonce,           // NEW: replay protection
//!     inventory_id     // NEW: cross-inventory protection
//! )

use ark_ff::PrimeField;
use ark_crypto_primitives::sponge::poseidon::{PoseidonConfig, PoseidonSponge};
use ark_crypto_primitives::sponge::{Absorb, CryptographicSponge};
use ark_r1cs_std::prelude::*;
use ark_r1cs_std::fields::fp::FpVar;
use ark_relations::r1cs::{ConstraintSystemRef, SynthesisError};
use ark_crypto_primitives::sponge::poseidon::constraints::PoseidonSpongeVar;
use ark_crypto_primitives::sponge::constraints::CryptographicSpongeVar;

/// Operation types for state transitions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum OpType {
    /// Deposit: add items to inventory
    Deposit = 0,
    /// Withdraw: remove items from inventory
    Withdraw = 1,
}

impl OpType {
    /// Convert to field element.
    pub fn to_field<F: PrimeField>(self) -> F {
        F::from(self as u64)
    }
}

/// Inputs for computing the signal hash.
#[derive(Clone, Debug)]
pub struct SignalInputs<F: PrimeField> {
    /// Old inventory commitment
    pub old_commitment: F,
    /// New inventory commitment
    pub new_commitment: F,
    /// Volume registry root (for item volume lookups)
    pub registry_root: F,
    /// Maximum capacity for the inventory
    pub max_capacity: u64,
    /// Item ID being operated on
    pub item_id: u64,
    /// Amount being deposited/withdrawn
    pub amount: u64,
    /// Operation type (deposit/withdraw)
    pub op_type: OpType,
    /// Current nonce from on-chain inventory (replay protection)
    pub nonce: u64,
    /// Inventory object ID as field element (cross-inventory protection)
    pub inventory_id: F,
}

impl<F: PrimeField + Absorb> SignalInputs<F> {
    /// Compute the signal hash from these inputs.
    pub fn compute_hash(&self, config: &PoseidonConfig<F>) -> F {
        let inputs = vec![
            self.old_commitment,
            self.new_commitment,
            self.registry_root,
            F::from(self.max_capacity),
            F::from(self.item_id),
            F::from(self.amount),
            self.op_type.to_field(),
            F::from(self.nonce),
            self.inventory_id,
        ];

        let mut sponge = PoseidonSponge::new(config);
        sponge.absorb(&inputs);
        sponge.squeeze_field_elements(1)[0]
    }
}

/// Circuit variable representation of signal inputs.
#[derive(Clone)]
pub struct SignalInputsVar<F: PrimeField> {
    /// Old inventory commitment
    pub old_commitment: FpVar<F>,
    /// New inventory commitment
    pub new_commitment: FpVar<F>,
    /// Volume registry root
    pub registry_root: FpVar<F>,
    /// Maximum capacity
    pub max_capacity: FpVar<F>,
    /// Item ID
    pub item_id: FpVar<F>,
    /// Amount
    pub amount: FpVar<F>,
    /// Operation type
    pub op_type: FpVar<F>,
    /// Nonce (replay protection)
    pub nonce: FpVar<F>,
    /// Inventory ID (cross-inventory protection)
    pub inventory_id: FpVar<F>,
}

impl<F: PrimeField> SignalInputsVar<F> {
    /// Create signal inputs from individual field variables.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        old_commitment: FpVar<F>,
        new_commitment: FpVar<F>,
        registry_root: FpVar<F>,
        max_capacity: FpVar<F>,
        item_id: FpVar<F>,
        amount: FpVar<F>,
        op_type: FpVar<F>,
        nonce: FpVar<F>,
        inventory_id: FpVar<F>,
    ) -> Self {
        Self {
            old_commitment,
            new_commitment,
            registry_root,
            max_capacity,
            item_id,
            amount,
            op_type,
            nonce,
            inventory_id,
        }
    }

    /// Compute the signal hash in-circuit.
    pub fn compute_hash(
        &self,
        cs: ConstraintSystemRef<F>,
        config: &PoseidonConfig<F>,
    ) -> Result<FpVar<F>, SynthesisError> {
        let inputs = vec![
            self.old_commitment.clone(),
            self.new_commitment.clone(),
            self.registry_root.clone(),
            self.max_capacity.clone(),
            self.item_id.clone(),
            self.amount.clone(),
            self.op_type.clone(),
            self.nonce.clone(),
            self.inventory_id.clone(),
        ];

        let mut sponge = PoseidonSpongeVar::new(cs, config);
        sponge.absorb(&inputs)?;
        let result = sponge.squeeze_field_elements(1)?;
        Ok(result[0].clone())
    }
}

/// Compute signal hash from raw field elements.
#[allow(clippy::too_many_arguments)]
pub fn compute_signal_hash<F: PrimeField + Absorb>(
    old_commitment: F,
    new_commitment: F,
    registry_root: F,
    max_capacity: u64,
    item_id: u64,
    amount: u64,
    op_type: OpType,
    nonce: u64,
    inventory_id: F,
    config: &PoseidonConfig<F>,
) -> F {
    let inputs = SignalInputs {
        old_commitment,
        new_commitment,
        registry_root,
        max_capacity,
        item_id,
        amount,
        op_type,
        nonce,
        inventory_id,
    };
    inputs.compute_hash(config)
}

/// Compute signal hash in-circuit.
#[allow(clippy::too_many_arguments)]
pub fn compute_signal_hash_var<F: PrimeField>(
    cs: ConstraintSystemRef<F>,
    old_commitment: &FpVar<F>,
    new_commitment: &FpVar<F>,
    registry_root: &FpVar<F>,
    max_capacity: &FpVar<F>,
    item_id: &FpVar<F>,
    amount: &FpVar<F>,
    op_type: &FpVar<F>,
    nonce: &FpVar<F>,
    inventory_id: &FpVar<F>,
    config: &PoseidonConfig<F>,
) -> Result<FpVar<F>, SynthesisError> {
    let inputs = SignalInputsVar::new(
        old_commitment.clone(),
        new_commitment.clone(),
        registry_root.clone(),
        max_capacity.clone(),
        item_id.clone(),
        amount.clone(),
        op_type.clone(),
        nonce.clone(),
        inventory_id.clone(),
    );
    inputs.compute_hash(cs, config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commitment::poseidon_config;
    use ark_bn254::Fr;
    use ark_relations::r1cs::ConstraintSystem;

    #[test]
    fn test_signal_hash_deterministic() {
        let config = poseidon_config::<Fr>();

        let hash1 = compute_signal_hash(
            Fr::from(100u64),  // old_commitment
            Fr::from(200u64),  // new_commitment
            Fr::from(300u64),  // registry_root
            1000,              // max_capacity
            42,                // item_id
            50,                // amount
            OpType::Deposit,
            0,                 // nonce
            Fr::from(999u64),  // inventory_id
            &config,
        );

        let hash2 = compute_signal_hash(
            Fr::from(100u64),
            Fr::from(200u64),
            Fr::from(300u64),
            1000,
            42,
            50,
            OpType::Deposit,
            0,
            Fr::from(999u64),
            &config,
        );

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_different_nonce_different_hash() {
        let config = poseidon_config::<Fr>();

        let hash1 = compute_signal_hash(
            Fr::from(100u64),
            Fr::from(200u64),
            Fr::from(300u64),
            1000,
            42,
            50,
            OpType::Deposit,
            0,  // nonce = 0
            Fr::from(999u64),
            &config,
        );

        let hash2 = compute_signal_hash(
            Fr::from(100u64),
            Fr::from(200u64),
            Fr::from(300u64),
            1000,
            42,
            50,
            OpType::Deposit,
            1,  // nonce = 1 (different!)
            Fr::from(999u64),
            &config,
        );

        assert_ne!(hash1, hash2, "Different nonces must produce different hashes (replay protection)");
    }

    #[test]
    fn test_different_inventory_id_different_hash() {
        let config = poseidon_config::<Fr>();

        let hash1 = compute_signal_hash(
            Fr::from(100u64),
            Fr::from(200u64),
            Fr::from(300u64),
            1000,
            42,
            50,
            OpType::Deposit,
            0,
            Fr::from(111u64),  // inventory A
            &config,
        );

        let hash2 = compute_signal_hash(
            Fr::from(100u64),
            Fr::from(200u64),
            Fr::from(300u64),
            1000,
            42,
            50,
            OpType::Deposit,
            0,
            Fr::from(222u64),  // inventory B (different!)
            &config,
        );

        assert_ne!(hash1, hash2, "Different inventory IDs must produce different hashes (cross-inventory protection)");
    }

    #[test]
    fn test_different_op_types_different_hashes() {
        let config = poseidon_config::<Fr>();

        let hash_deposit = compute_signal_hash(
            Fr::from(100u64),
            Fr::from(200u64),
            Fr::from(300u64),
            1000,
            42,
            50,
            OpType::Deposit,
            0,
            Fr::from(999u64),
            &config,
        );

        let hash_withdraw = compute_signal_hash(
            Fr::from(100u64),
            Fr::from(200u64),
            Fr::from(300u64),
            1000,
            42,
            50,
            OpType::Withdraw,
            0,
            Fr::from(999u64),
            &config,
        );

        assert_ne!(hash_deposit, hash_withdraw);
    }

    #[test]
    fn test_in_circuit_matches_native() {
        let config = poseidon_config::<Fr>();

        let old_commitment = Fr::from(100u64);
        let new_commitment = Fr::from(200u64);
        let registry_root = Fr::from(300u64);
        let max_capacity = 1000u64;
        let item_id = 42u64;
        let amount = 50u64;
        let op_type = OpType::Deposit;
        let nonce = 5u64;
        let inventory_id = Fr::from(999u64);

        // Compute native
        let native_hash = compute_signal_hash(
            old_commitment,
            new_commitment,
            registry_root,
            max_capacity,
            item_id,
            amount,
            op_type,
            nonce,
            inventory_id,
            &config,
        );

        // Compute in-circuit
        let cs = ConstraintSystem::<Fr>::new_ref();

        let old_commitment_var = FpVar::new_witness(cs.clone(), || Ok(old_commitment)).unwrap();
        let new_commitment_var = FpVar::new_witness(cs.clone(), || Ok(new_commitment)).unwrap();
        let registry_root_var = FpVar::new_witness(cs.clone(), || Ok(registry_root)).unwrap();
        let max_capacity_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(max_capacity))).unwrap();
        let item_id_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(item_id))).unwrap();
        let amount_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(amount))).unwrap();
        let op_type_var = FpVar::new_witness(cs.clone(), || Ok(op_type.to_field::<Fr>())).unwrap();
        let nonce_var = FpVar::new_witness(cs.clone(), || Ok(Fr::from(nonce))).unwrap();
        let inventory_id_var = FpVar::new_witness(cs.clone(), || Ok(inventory_id)).unwrap();

        let circuit_hash = compute_signal_hash_var(
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
            &config,
        )
        .unwrap();

        // Verify they match
        let expected_var = FpVar::new_input(cs.clone(), || Ok(native_hash)).unwrap();
        circuit_hash.enforce_equal(&expected_var).unwrap();

        assert!(cs.is_satisfied().unwrap());
        println!("Signal hash constraints: {}", cs.num_constraints());
    }
}
