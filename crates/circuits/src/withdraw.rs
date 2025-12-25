//! WithdrawCircuit: Proves a valid withdrawal from an inventory with state transition.

use std::sync::Arc;

use ark_crypto_primitives::sponge::poseidon::PoseidonConfig;
use ark_ff::PrimeField;
use ark_r1cs_std::{
    alloc::AllocVar,
    fields::{fp::FpVar, FieldVar},
    prelude::*,
};
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};

use crate::commitment::PoseidonGadget;
use crate::inventory::{Inventory, InventoryVar, MAX_ITEM_SLOTS};

/// Circuit that proves: "old_inventory - amount = new_inventory, both commitments valid"
///
/// Public inputs:
/// - old_commitment: Commitment to the old inventory state
/// - new_commitment: Commitment to the new inventory state
/// - item_id: The item being withdrawn
/// - amount: The amount being withdrawn
///
/// Private witnesses:
/// - old_inventory: The inventory before withdrawal
/// - new_inventory: The inventory after withdrawal
/// - old_blinding: Blinding factor for old commitment
/// - new_blinding: Blinding factor for new commitment
#[derive(Clone)]
pub struct WithdrawCircuit<F: PrimeField> {
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
    /// Public: Item ID being withdrawn
    pub item_id: u32,
    /// Public: Amount being withdrawn
    pub amount: u64,

    /// Poseidon configuration
    pub poseidon_config: Arc<PoseidonConfig<F>>,
}

impl<F: PrimeField> WithdrawCircuit<F> {
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
    for WithdrawCircuit<F>
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

        // 5. Verify old_inventory[item_id] >= amount
        let old_quantity = old_inv_var.get_quantity_for_item(cs.clone(), &item_id_var)?;
        let difference = &old_quantity - &amount_var;
        difference.enforce_cmp(&FpVar::zero(), std::cmp::Ordering::Greater, true)?;

        // 6. Verify new_inventory[item_id] = old_inventory[item_id] - amount
        let new_quantity = new_inv_var.get_quantity_for_item(cs.clone(), &item_id_var)?;
        let expected_new_quantity = &old_quantity - &amount_var;
        new_quantity.enforce_equal(&expected_new_quantity)?;

        // 7. Verify all other slots unchanged
        for i in 0..MAX_ITEM_SLOTS {
            let (old_id, old_qty) = &old_inv_var.slots[i];
            let (new_id, new_qty) = &new_inv_var.slots[i];

            // Check if this slot contains the target item
            let is_target_slot = old_id.is_eq(&item_id_var)?;

            // For non-target slots, both id and quantity must be unchanged
            let id_unchanged = old_id.is_eq(new_id)?;
            let qty_unchanged = old_qty.is_eq(new_qty)?;

            // If not target slot, enforce unchanged
            // is_target_slot OR (id_unchanged AND qty_unchanged)
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
    fn test_withdraw_valid() {
        let config = Arc::new(poseidon_config::<Fr>());

        // Start with 100 swords
        let old_inventory = Inventory::from_items(&[(1, 100), (2, 50)]);
        let old_blinding = Fr::from(12345u64);
        let old_commitment = create_inventory_commitment(&old_inventory, old_blinding, &config);

        // Withdraw 30 swords
        let mut new_inventory = old_inventory.clone();
        new_inventory.withdraw(1, 30).unwrap();
        let new_blinding = Fr::from(67890u64);
        let new_commitment = create_inventory_commitment(&new_inventory, new_blinding, &config);

        let circuit = WithdrawCircuit::new(
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
        println!("Withdraw circuit constraints: {}", cs.num_constraints());
    }

    #[test]
    fn test_withdraw_insufficient_balance() {
        let config = Arc::new(poseidon_config::<Fr>());

        let old_inventory = Inventory::from_items(&[(1, 50)]);
        let old_blinding = Fr::from(12345u64);
        let old_commitment = create_inventory_commitment(&old_inventory, old_blinding, &config);

        // Try to withdraw more than available (manipulated new_inventory)
        let new_inventory = Inventory::from_items(&[(1, 0)]); // Pretend we withdrew 100
        let new_blinding = Fr::from(67890u64);
        let new_commitment = create_inventory_commitment(&new_inventory, new_blinding, &config);

        let circuit = WithdrawCircuit::new(
            old_inventory,
            new_inventory,
            old_blinding,
            new_blinding,
            old_commitment,
            new_commitment,
            1,   // item_id
            100, // amount (more than we have)
            config,
        );

        let cs = ConstraintSystem::<Fr>::new_ref();
        circuit.generate_constraints(cs.clone()).unwrap();

        // Should NOT be satisfied - insufficient balance
        assert!(!cs.is_satisfied().unwrap());
    }
}
