//! TransferCircuit: Proves a valid transfer between two inventories.

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

/// Circuit that proves: "src -= amount, dst += amount, all commitments valid"
///
/// Public inputs:
/// - src_old_commitment: Source inventory before transfer
/// - src_new_commitment: Source inventory after transfer
/// - dst_old_commitment: Destination inventory before transfer
/// - dst_new_commitment: Destination inventory after transfer
/// - item_id: The item being transferred
/// - amount: The amount being transferred
///
/// Private witnesses:
/// - src_old_inventory, src_new_inventory: Source inventory states
/// - dst_old_inventory, dst_new_inventory: Destination inventory states
/// - src_old_blinding, src_new_blinding: Source blinding factors
/// - dst_old_blinding, dst_new_blinding: Destination blinding factors
#[derive(Clone)]
pub struct TransferCircuit<F: PrimeField> {
    // Source inventory (private)
    pub src_old_inventory: Option<Inventory>,
    pub src_new_inventory: Option<Inventory>,
    pub src_old_blinding: Option<F>,
    pub src_new_blinding: Option<F>,

    // Destination inventory (private)
    pub dst_old_inventory: Option<Inventory>,
    pub dst_new_inventory: Option<Inventory>,
    pub dst_old_blinding: Option<F>,
    pub dst_new_blinding: Option<F>,

    // Public inputs
    pub src_old_commitment: Option<F>,
    pub src_new_commitment: Option<F>,
    pub dst_old_commitment: Option<F>,
    pub dst_new_commitment: Option<F>,
    pub item_id: u32,
    pub amount: u64,

    pub poseidon_config: Arc<PoseidonConfig<F>>,
}

impl<F: PrimeField> TransferCircuit<F> {
    /// Create a new circuit instance for proving.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        src_old_inventory: Inventory,
        src_new_inventory: Inventory,
        src_old_blinding: F,
        src_new_blinding: F,
        dst_old_inventory: Inventory,
        dst_new_inventory: Inventory,
        dst_old_blinding: F,
        dst_new_blinding: F,
        src_old_commitment: F,
        src_new_commitment: F,
        dst_old_commitment: F,
        dst_new_commitment: F,
        item_id: u32,
        amount: u64,
        poseidon_config: Arc<PoseidonConfig<F>>,
    ) -> Self {
        Self {
            src_old_inventory: Some(src_old_inventory),
            src_new_inventory: Some(src_new_inventory),
            src_old_blinding: Some(src_old_blinding),
            src_new_blinding: Some(src_new_blinding),
            dst_old_inventory: Some(dst_old_inventory),
            dst_new_inventory: Some(dst_new_inventory),
            dst_old_blinding: Some(dst_old_blinding),
            dst_new_blinding: Some(dst_new_blinding),
            src_old_commitment: Some(src_old_commitment),
            src_new_commitment: Some(src_new_commitment),
            dst_old_commitment: Some(dst_old_commitment),
            dst_new_commitment: Some(dst_new_commitment),
            item_id,
            amount,
            poseidon_config,
        }
    }

    /// Create an empty circuit for setup.
    pub fn empty(poseidon_config: Arc<PoseidonConfig<F>>) -> Self {
        Self {
            src_old_inventory: None,
            src_new_inventory: None,
            src_old_blinding: None,
            src_new_blinding: None,
            dst_old_inventory: None,
            dst_new_inventory: None,
            dst_old_blinding: None,
            dst_new_blinding: None,
            src_old_commitment: None,
            src_new_commitment: None,
            dst_old_commitment: None,
            dst_new_commitment: None,
            item_id: 0,
            amount: 0,
            poseidon_config,
        }
    }
}

impl<F: PrimeField + ark_crypto_primitives::sponge::Absorb> ConstraintSynthesizer<F>
    for TransferCircuit<F>
{
    fn generate_constraints(self, cs: ConstraintSystemRef<F>) -> Result<(), SynthesisError> {
        let poseidon = PoseidonGadget::new((*self.poseidon_config).clone());

        // === Source inventory witnesses ===
        let src_old_inv = self.src_old_inventory.unwrap_or_default();
        let src_new_inv = self.src_new_inventory.unwrap_or_default();
        let src_old_inv_var = InventoryVar::new_witness(cs.clone(), &src_old_inv)?;
        let src_new_inv_var = InventoryVar::new_witness(cs.clone(), &src_new_inv)?;

        let src_old_blinding_var = FpVar::new_witness(cs.clone(), || {
            self.src_old_blinding.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let src_new_blinding_var = FpVar::new_witness(cs.clone(), || {
            self.src_new_blinding.ok_or(SynthesisError::AssignmentMissing)
        })?;

        // === Destination inventory witnesses ===
        let dst_old_inv = self.dst_old_inventory.unwrap_or_default();
        let dst_new_inv = self.dst_new_inventory.unwrap_or_default();
        let dst_old_inv_var = InventoryVar::new_witness(cs.clone(), &dst_old_inv)?;
        let dst_new_inv_var = InventoryVar::new_witness(cs.clone(), &dst_new_inv)?;

        let dst_old_blinding_var = FpVar::new_witness(cs.clone(), || {
            self.dst_old_blinding.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let dst_new_blinding_var = FpVar::new_witness(cs.clone(), || {
            self.dst_new_blinding.ok_or(SynthesisError::AssignmentMissing)
        })?;

        // === Public inputs ===
        let src_old_commitment_var = FpVar::new_input(cs.clone(), || {
            self.src_old_commitment.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let src_new_commitment_var = FpVar::new_input(cs.clone(), || {
            self.src_new_commitment.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let dst_old_commitment_var = FpVar::new_input(cs.clone(), || {
            self.dst_old_commitment.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let dst_new_commitment_var = FpVar::new_input(cs.clone(), || {
            self.dst_new_commitment.ok_or(SynthesisError::AssignmentMissing)
        })?;

        let item_id_var = FpVar::new_input(cs.clone(), || Ok(F::from(self.item_id as u64)))?;
        let amount_var = FpVar::new_input(cs.clone(), || Ok(F::from(self.amount)))?;

        // === Verify all four commitments ===
        let computed_src_old = poseidon.commit_inventory(
            cs.clone(),
            &src_old_inv_var.to_field_vars(),
            &src_old_blinding_var,
        )?;
        computed_src_old.enforce_equal(&src_old_commitment_var)?;

        let computed_src_new = poseidon.commit_inventory(
            cs.clone(),
            &src_new_inv_var.to_field_vars(),
            &src_new_blinding_var,
        )?;
        computed_src_new.enforce_equal(&src_new_commitment_var)?;

        let computed_dst_old = poseidon.commit_inventory(
            cs.clone(),
            &dst_old_inv_var.to_field_vars(),
            &dst_old_blinding_var,
        )?;
        computed_dst_old.enforce_equal(&dst_old_commitment_var)?;

        let computed_dst_new = poseidon.commit_inventory(
            cs.clone(),
            &dst_new_inv_var.to_field_vars(),
            &dst_new_blinding_var,
        )?;
        computed_dst_new.enforce_equal(&dst_new_commitment_var)?;

        // === Verify source withdrawal ===
        let src_old_qty = src_old_inv_var.get_quantity_for_item(cs.clone(), &item_id_var)?;
        let src_new_qty = src_new_inv_var.get_quantity_for_item(cs.clone(), &item_id_var)?;

        // src_old >= amount
        let src_diff = &src_old_qty - &amount_var;
        src_diff.enforce_cmp(&FpVar::zero(), std::cmp::Ordering::Greater, true)?;

        // src_new = src_old - amount
        let expected_src_new = &src_old_qty - &amount_var;
        src_new_qty.enforce_equal(&expected_src_new)?;

        // === Verify destination deposit ===
        let dst_old_qty = dst_old_inv_var.get_quantity_for_item(cs.clone(), &item_id_var)?;
        let dst_new_qty = dst_new_inv_var.get_quantity_for_item(cs.clone(), &item_id_var)?;

        // dst_new = dst_old + amount
        let expected_dst_new = &dst_old_qty + &amount_var;
        dst_new_qty.enforce_equal(&expected_dst_new)?;

        // === Verify other slots unchanged in both inventories ===
        // Source inventory
        for i in 0..MAX_ITEM_SLOTS {
            let (old_id, old_qty) = &src_old_inv_var.slots[i];
            let (new_id, new_qty) = &src_new_inv_var.slots[i];

            let is_target_slot = old_id.is_eq(&item_id_var)?;
            let id_unchanged = old_id.is_eq(new_id)?;
            let qty_unchanged = old_qty.is_eq(new_qty)?;

            let slot_valid = is_target_slot.or(&id_unchanged.and(&qty_unchanged)?)?;
            slot_valid.enforce_equal(&Boolean::TRUE)?;
        }

        // Destination inventory
        for i in 0..MAX_ITEM_SLOTS {
            let (old_id, old_qty) = &dst_old_inv_var.slots[i];
            let (new_id, new_qty) = &dst_new_inv_var.slots[i];

            let is_target_in_old = old_id.is_eq(&item_id_var)?;
            let is_target_in_new = new_id.is_eq(&item_id_var)?;
            let is_target_slot = is_target_in_old.or(&is_target_in_new)?;

            let id_unchanged = old_id.is_eq(new_id)?;
            let qty_unchanged = old_qty.is_eq(new_qty)?;

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
    fn test_transfer_valid() {
        let config = Arc::new(poseidon_config::<Fr>());

        // Source: 100 gold
        let src_old = Inventory::from_items(&[(1, 100)]);
        let src_old_blinding = Fr::from(111u64);
        let src_old_commitment = create_inventory_commitment(&src_old, src_old_blinding, &config);

        // Destination: 20 gold
        let dst_old = Inventory::from_items(&[(1, 20)]);
        let dst_old_blinding = Fr::from(222u64);
        let dst_old_commitment = create_inventory_commitment(&dst_old, dst_old_blinding, &config);

        // Transfer 30 gold
        let mut src_new = src_old.clone();
        src_new.withdraw(1, 30).unwrap();
        let src_new_blinding = Fr::from(333u64);
        let src_new_commitment = create_inventory_commitment(&src_new, src_new_blinding, &config);

        let mut dst_new = dst_old.clone();
        dst_new.deposit(1, 30).unwrap();
        let dst_new_blinding = Fr::from(444u64);
        let dst_new_commitment = create_inventory_commitment(&dst_new, dst_new_blinding, &config);

        let circuit = TransferCircuit::new(
            src_old,
            src_new,
            src_old_blinding,
            src_new_blinding,
            dst_old,
            dst_new,
            dst_old_blinding,
            dst_new_blinding,
            src_old_commitment,
            src_new_commitment,
            dst_old_commitment,
            dst_new_commitment,
            1,  // item_id (gold)
            30, // amount
            config,
        );

        let cs = ConstraintSystem::<Fr>::new_ref();
        circuit.generate_constraints(cs.clone()).unwrap();

        assert!(cs.is_satisfied().unwrap());
        println!("Transfer circuit constraints: {}", cs.num_constraints());
    }

    #[test]
    fn test_transfer_insufficient_source() {
        let config = Arc::new(poseidon_config::<Fr>());

        // Source: only 10 gold
        let src_old = Inventory::from_items(&[(1, 10)]);
        let src_old_blinding = Fr::from(111u64);
        let src_old_commitment = create_inventory_commitment(&src_old, src_old_blinding, &config);

        // Destination: empty
        let dst_old = Inventory::new();
        let dst_old_blinding = Fr::from(222u64);
        let dst_old_commitment = create_inventory_commitment(&dst_old, dst_old_blinding, &config);

        // Try to transfer 50 gold (more than source has)
        // Fabricate invalid new states
        let src_new = Inventory::from_items(&[(1, 0)]); // Pretend it worked
        let src_new_blinding = Fr::from(333u64);
        let src_new_commitment = create_inventory_commitment(&src_new, src_new_blinding, &config);

        let dst_new = Inventory::from_items(&[(1, 50)]);
        let dst_new_blinding = Fr::from(444u64);
        let dst_new_commitment = create_inventory_commitment(&dst_new, dst_new_blinding, &config);

        let circuit = TransferCircuit::new(
            src_old,
            src_new,
            src_old_blinding,
            src_new_blinding,
            dst_old,
            dst_new,
            dst_old_blinding,
            dst_new_blinding,
            src_old_commitment,
            src_new_commitment,
            dst_old_commitment,
            dst_new_commitment,
            1,  // item_id
            50, // amount (more than source has)
            config,
        );

        let cs = ConstraintSystem::<Fr>::new_ref();
        circuit.generate_constraints(cs.clone()).unwrap();

        // Should NOT be satisfied
        assert!(!cs.is_satisfied().unwrap());
    }
}
