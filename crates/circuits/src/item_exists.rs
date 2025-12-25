//! ItemExistsCircuit: Proves that an inventory contains at least a minimum quantity of an item.

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
use crate::inventory::{Inventory, InventoryVar};

/// Circuit that proves: "Commitment contains >= min_quantity of item_id"
///
/// Public inputs:
/// - commitment: The Poseidon hash of the inventory
/// - item_id: The item to check
/// - min_quantity: The minimum quantity required
///
/// Private witnesses:
/// - inventory: The actual inventory contents
/// - blinding: The blinding factor used in the commitment
#[derive(Clone)]
pub struct ItemExistsCircuit<F: PrimeField> {
    /// Private: The inventory contents
    pub inventory: Option<Inventory>,
    /// Private: The blinding factor
    pub blinding: Option<F>,

    /// Public: The commitment to verify against
    pub commitment: Option<F>,
    /// Public: The item ID to check
    pub item_id: u32,
    /// Public: The minimum quantity required
    pub min_quantity: u64,

    /// Poseidon configuration
    pub poseidon_config: Arc<PoseidonConfig<F>>,
}

impl<F: PrimeField> ItemExistsCircuit<F> {
    /// Create a new circuit instance for proving.
    pub fn new(
        inventory: Inventory,
        blinding: F,
        commitment: F,
        item_id: u32,
        min_quantity: u64,
        poseidon_config: Arc<PoseidonConfig<F>>,
    ) -> Self {
        Self {
            inventory: Some(inventory),
            blinding: Some(blinding),
            commitment: Some(commitment),
            item_id,
            min_quantity,
            poseidon_config,
        }
    }

    /// Create an empty circuit for setup (constraint generation only).
    pub fn empty(poseidon_config: Arc<PoseidonConfig<F>>) -> Self {
        Self {
            inventory: None,
            blinding: None,
            commitment: None,
            item_id: 0,
            min_quantity: 0,
            poseidon_config,
        }
    }
}

impl<F: PrimeField + ark_crypto_primitives::sponge::Absorb> ConstraintSynthesizer<F>
    for ItemExistsCircuit<F>
{
    fn generate_constraints(self, cs: ConstraintSystemRef<F>) -> Result<(), SynthesisError> {
        // 1. Allocate private witnesses
        let inventory = self.inventory.unwrap_or_default();
        let inventory_var = InventoryVar::new_witness(cs.clone(), &inventory)?;

        let blinding_var = FpVar::new_witness(cs.clone(), || {
            self.blinding.ok_or(SynthesisError::AssignmentMissing)
        })?;

        // 2. Allocate public inputs
        let commitment_var = FpVar::new_input(cs.clone(), || {
            self.commitment.ok_or(SynthesisError::AssignmentMissing)
        })?;

        let item_id_var = FpVar::new_input(cs.clone(), || Ok(F::from(self.item_id as u64)))?;

        let min_quantity_var = FpVar::new_input(cs.clone(), || Ok(F::from(self.min_quantity)))?;

        // 3. Verify commitment: Poseidon(inventory, blinding) == commitment
        let poseidon = PoseidonGadget::new((*self.poseidon_config).clone());
        let computed_commitment =
            poseidon.commit_inventory(cs.clone(), &inventory_var.to_field_vars(), &blinding_var)?;

        computed_commitment.enforce_equal(&commitment_var)?;

        // 4. Get the quantity for the target item
        let actual_quantity =
            inventory_var.get_quantity_for_item(cs.clone(), &item_id_var)?;

        // 5. Verify: actual_quantity >= min_quantity
        // This is equivalent to: actual_quantity - min_quantity >= 0
        // We prove this by showing (actual - min) can be represented as a non-negative value
        let difference = &actual_quantity - &min_quantity_var;

        // Enforce that difference is non-negative by checking it fits in a reasonable bit width
        // For a 64-bit quantity, the difference should fit in 64 bits if valid
        difference.enforce_cmp(&FpVar::zero(), std::cmp::Ordering::Greater, true)?;

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
    fn test_item_exists_valid() {
        let config = Arc::new(poseidon_config::<Fr>());
        let inventory = Inventory::from_items(&[(1, 100), (2, 50)]);
        let blinding = Fr::from(12345u64);
        let commitment = create_inventory_commitment(&inventory, blinding, &config);

        // Prove we have >= 50 of item 1
        let circuit = ItemExistsCircuit::new(
            inventory,
            blinding,
            commitment,
            1,  // item_id
            50, // min_quantity
            config,
        );

        let cs = ConstraintSystem::<Fr>::new_ref();
        circuit.generate_constraints(cs.clone()).unwrap();

        assert!(cs.is_satisfied().unwrap());
        println!("Constraints: {}", cs.num_constraints());
    }

    #[test]
    fn test_item_exists_exact() {
        let config = Arc::new(poseidon_config::<Fr>());
        let inventory = Inventory::from_items(&[(1, 100)]);
        let blinding = Fr::from(12345u64);
        let commitment = create_inventory_commitment(&inventory, blinding, &config);

        // Prove we have >= 100 of item 1 (exact match)
        let circuit = ItemExistsCircuit::new(
            inventory,
            blinding,
            commitment,
            1,   // item_id
            100, // min_quantity (exact)
            config,
        );

        let cs = ConstraintSystem::<Fr>::new_ref();
        circuit.generate_constraints(cs.clone()).unwrap();

        assert!(cs.is_satisfied().unwrap());
    }

    #[test]
    fn test_item_exists_insufficient() {
        let config = Arc::new(poseidon_config::<Fr>());
        let inventory = Inventory::from_items(&[(1, 50)]);
        let blinding = Fr::from(12345u64);
        let commitment = create_inventory_commitment(&inventory, blinding, &config);

        // Try to prove we have >= 100 of item 1 (should fail)
        let circuit = ItemExistsCircuit::new(
            inventory,
            blinding,
            commitment,
            1,   // item_id
            100, // min_quantity (more than we have)
            config,
        );

        let cs = ConstraintSystem::<Fr>::new_ref();
        circuit.generate_constraints(cs.clone()).unwrap();

        // Constraint system should NOT be satisfied
        assert!(!cs.is_satisfied().unwrap());
    }
}
