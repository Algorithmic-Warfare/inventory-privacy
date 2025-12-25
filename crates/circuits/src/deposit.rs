//! DepositCircuit: Proves a valid deposit into an inventory with state transition.

use std::sync::Arc;

use ark_crypto_primitives::sponge::poseidon::PoseidonConfig;
use ark_ff::PrimeField;
use ark_r1cs_std::{
    alloc::AllocVar,
    fields::fp::FpVar,
    prelude::*,
};
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};

use crate::commitment::PoseidonGadget;
use crate::inventory::{Inventory, InventoryVar, MAX_ITEM_SLOTS};

/// Circuit that proves: "old_inventory + amount = new_inventory, both commitments valid"
///
/// Public inputs:
/// - old_commitment: Commitment to the old inventory state
/// - new_commitment: Commitment to the new inventory state
/// - item_id: The item being deposited
/// - amount: The amount being deposited
///
/// Private witnesses:
/// - old_inventory: The inventory before deposit
/// - new_inventory: The inventory after deposit
/// - old_blinding: Blinding factor for old commitment
/// - new_blinding: Blinding factor for new commitment
#[derive(Clone)]
pub struct DepositCircuit<F: PrimeField> {
    /// Private: Old inventory contents
    pub old_inventory: Option<Inventory>,
    /// Private: New inventory contents
    pub new_inventory: Option<Inventory>,
    /// Private: Old blinding factor
    pub old_blinding: Option<F>,
    /// Private: New blinding factor
    pub new_blinding: Option<F>,

    /// Public: Old commitment
    pub old_commitment: Option<F>,
    /// Public: New commitment
    pub new_commitment: Option<F>,
    /// Public: Item ID being deposited
    pub item_id: u32,
    /// Public: Amount being deposited
    pub amount: u64,

    /// Poseidon configuration
    pub poseidon_config: Arc<PoseidonConfig<F>>,
}

impl<F: PrimeField> DepositCircuit<F> {
    /// Create a new circuit instance for proving.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        old_inventory: Inventory,
        new_inventory: Inventory,
        old_blinding: F,
        new_blinding: F,
        old_commitment: F,
        new_commitment: F,
        item_id: u32,
        amount: u64,
        poseidon_config: Arc<PoseidonConfig<F>>,
    ) -> Self {
        Self {
            old_inventory: Some(old_inventory),
            new_inventory: Some(new_inventory),
            old_blinding: Some(old_blinding),
            new_blinding: Some(new_blinding),
            old_commitment: Some(old_commitment),
            new_commitment: Some(new_commitment),
            item_id,
            amount,
            poseidon_config,
        }
    }

    /// Create an empty circuit for setup.
    pub fn empty(poseidon_config: Arc<PoseidonConfig<F>>) -> Self {
        Self {
            old_inventory: None,
            new_inventory: None,
            old_blinding: None,
            new_blinding: None,
            old_commitment: None,
            new_commitment: None,
            item_id: 0,
            amount: 0,
            poseidon_config,
        }
    }
}

impl<F: PrimeField + ark_crypto_primitives::sponge::Absorb> ConstraintSynthesizer<F>
    for DepositCircuit<F>
{
    fn generate_constraints(self, cs: ConstraintSystemRef<F>) -> Result<(), SynthesisError> {
        // 1. Allocate private witnesses
        let old_inventory = self.old_inventory.unwrap_or_default();
        let new_inventory = self.new_inventory.unwrap_or_default();

        let old_inv_var = InventoryVar::new_witness(cs.clone(), &old_inventory)?;
        let new_inv_var = InventoryVar::new_witness(cs.clone(), &new_inventory)?;

        let old_blinding_var = FpVar::new_witness(cs.clone(), || {
            self.old_blinding.ok_or(SynthesisError::AssignmentMissing)
        })?;

        let new_blinding_var = FpVar::new_witness(cs.clone(), || {
            self.new_blinding.ok_or(SynthesisError::AssignmentMissing)
        })?;

        // 2. Allocate public inputs
        let old_commitment_var = FpVar::new_input(cs.clone(), || {
            self.old_commitment.ok_or(SynthesisError::AssignmentMissing)
        })?;

        let new_commitment_var = FpVar::new_input(cs.clone(), || {
            self.new_commitment.ok_or(SynthesisError::AssignmentMissing)
        })?;

        let item_id_var = FpVar::new_input(cs.clone(), || Ok(F::from(self.item_id as u64)))?;

        let amount_var = FpVar::new_input(cs.clone(), || Ok(F::from(self.amount)))?;

        // 3. Verify old commitment
        let poseidon = PoseidonGadget::new((*self.poseidon_config).clone());
        let computed_old =
            poseidon.commit_inventory(cs.clone(), &old_inv_var.to_field_vars(), &old_blinding_var)?;
        computed_old.enforce_equal(&old_commitment_var)?;

        // 4. Verify new commitment
        let computed_new =
            poseidon.commit_inventory(cs.clone(), &new_inv_var.to_field_vars(), &new_blinding_var)?;
        computed_new.enforce_equal(&new_commitment_var)?;

        // 5. Get old and new quantities
        let old_quantity = old_inv_var.get_quantity_for_item(cs.clone(), &item_id_var)?;
        let new_quantity = new_inv_var.get_quantity_for_item(cs.clone(), &item_id_var)?;

        // 6. Verify new_inventory[item_id] = old_inventory[item_id] + amount
        let expected_new_quantity = &old_quantity + &amount_var;
        new_quantity.enforce_equal(&expected_new_quantity)?;

        // 7. Verify all other slots unchanged (or new slot created for new item)
        // For simplicity in PoC, we verify slot-by-slot with conditional logic
        for i in 0..MAX_ITEM_SLOTS {
            let (old_id, old_qty) = &old_inv_var.slots[i];
            let (new_id, new_qty) = &new_inv_var.slots[i];

            // Check if this slot contains the target item (in old or new)
            let is_target_in_old = old_id.is_eq(&item_id_var)?;
            let is_target_in_new = new_id.is_eq(&item_id_var)?;
            let is_target_slot = is_target_in_old.or(&is_target_in_new)?;

            // For non-target slots, both id and quantity must be unchanged
            let id_unchanged = old_id.is_eq(new_id)?;
            let qty_unchanged = old_qty.is_eq(new_qty)?;

            // If not target slot, enforce unchanged
            let slot_valid = is_target_slot.or(&id_unchanged.and(&qty_unchanged)?)?;
            slot_valid.enforce_equal(&Boolean::TRUE)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commitment::{create_inventory_commitment, poseidon_config};
    use ark_bn254::Fr;
    use ark_relations::r1cs::ConstraintSystem;

    #[test]
    fn test_deposit_existing_item() {
        let config = Arc::new(poseidon_config::<Fr>());

        // Start with 50 swords
        let old_inventory = Inventory::from_items(&[(1, 50)]);
        let old_blinding = Fr::from(12345u64);
        let old_commitment = create_inventory_commitment(&old_inventory, old_blinding, &config);

        // Deposit 30 more swords
        let mut new_inventory = old_inventory.clone();
        new_inventory.deposit(1, 30).unwrap();
        let new_blinding = Fr::from(67890u64);
        let new_commitment = create_inventory_commitment(&new_inventory, new_blinding, &config);

        let circuit = DepositCircuit::new(
            old_inventory,
            new_inventory,
            old_blinding,
            new_blinding,
            old_commitment,
            new_commitment,
            1,  // item_id
            30, // amount
            config,
        );

        let cs = ConstraintSystem::<Fr>::new_ref();
        circuit.generate_constraints(cs.clone()).unwrap();

        assert!(cs.is_satisfied().unwrap());
        println!("Deposit circuit constraints: {}", cs.num_constraints());
    }

    #[test]
    fn test_deposit_new_item() {
        let config = Arc::new(poseidon_config::<Fr>());

        // Start empty
        let old_inventory = Inventory::new();
        let old_blinding = Fr::from(12345u64);
        let old_commitment = create_inventory_commitment(&old_inventory, old_blinding, &config);

        // Deposit 100 of a new item
        let mut new_inventory = old_inventory.clone();
        new_inventory.deposit(42, 100).unwrap();
        let new_blinding = Fr::from(67890u64);
        let new_commitment = create_inventory_commitment(&new_inventory, new_blinding, &config);

        let circuit = DepositCircuit::new(
            old_inventory,
            new_inventory,
            old_blinding,
            new_blinding,
            old_commitment,
            new_commitment,
            42,  // item_id
            100, // amount
            config,
        );

        let cs = ConstraintSystem::<Fr>::new_ref();
        circuit.generate_constraints(cs.clone()).unwrap();

        assert!(cs.is_satisfied().unwrap());
    }

    #[test]
    fn test_deposit_wrong_amount() {
        let config = Arc::new(poseidon_config::<Fr>());

        let old_inventory = Inventory::from_items(&[(1, 50)]);
        let old_blinding = Fr::from(12345u64);
        let old_commitment = create_inventory_commitment(&old_inventory, old_blinding, &config);

        // Claim to deposit 30 but actually deposit 50
        let mut new_inventory = old_inventory.clone();
        new_inventory.deposit(1, 50).unwrap(); // Actually depositing 50
        let new_blinding = Fr::from(67890u64);
        let new_commitment = create_inventory_commitment(&new_inventory, new_blinding, &config);

        let circuit = DepositCircuit::new(
            old_inventory,
            new_inventory,
            old_blinding,
            new_blinding,
            old_commitment,
            new_commitment,
            1,  // item_id
            30, // Claiming 30 but new_inventory has +50
            config,
        );

        let cs = ConstraintSystem::<Fr>::new_ref();
        circuit.generate_constraints(cs.clone()).unwrap();

        // Should NOT be satisfied - amount mismatch
        assert!(!cs.is_satisfied().unwrap());
    }
}
