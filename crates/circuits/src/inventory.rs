//! Inventory data structures for ZK circuits.

use ark_ff::PrimeField;
use ark_r1cs_std::{
    alloc::AllocVar,
    fields::fp::FpVar,
    prelude::*,
};
use ark_relations::r1cs::{ConstraintSystemRef, SynthesisError};

/// Maximum number of item slots in an inventory.
/// Using 16 slots for the PoC (can be adjusted based on performance requirements).
pub const MAX_ITEM_SLOTS: usize = 16;

/// A slot in the inventory containing an item ID and quantity.
/// item_id = 0 means the slot is empty.
#[derive(Clone, Debug, Default)]
pub struct ItemSlot {
    pub item_id: u32,
    pub quantity: u64,
}

/// Fixed-slot inventory structure.
/// Simple and efficient for ZK circuits.
#[derive(Clone, Debug)]
pub struct Inventory {
    /// Slots: (item_id, quantity) pairs
    /// item_id = 0 means empty slot
    pub slots: [ItemSlot; MAX_ITEM_SLOTS],
}

impl Default for Inventory {
    fn default() -> Self {
        Self {
            slots: std::array::from_fn(|_| ItemSlot::default()),
        }
    }
}

impl Inventory {
    /// Create a new empty inventory.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an inventory from a list of (item_id, quantity) pairs.
    pub fn from_items(items: &[(u32, u64)]) -> Self {
        let mut inventory = Self::new();
        for (i, (item_id, quantity)) in items.iter().take(MAX_ITEM_SLOTS).enumerate() {
            inventory.slots[i] = ItemSlot {
                item_id: *item_id,
                quantity: *quantity,
            };
        }
        inventory
    }

    /// Convert inventory to field elements for Poseidon hashing.
    pub fn to_field_elements<F: PrimeField>(&self) -> Vec<F> {
        self.slots
            .iter()
            .flat_map(|slot| {
                vec![
                    F::from(slot.item_id as u64),
                    F::from(slot.quantity),
                ]
            })
            .collect()
    }

    /// Get the quantity of a specific item.
    pub fn get_quantity(&self, item_id: u32) -> u64 {
        self.slots
            .iter()
            .find(|slot| slot.item_id == item_id)
            .map(|slot| slot.quantity)
            .unwrap_or(0)
    }

    /// Find the slot index for a given item ID.
    pub fn find_slot(&self, item_id: u32) -> Option<usize> {
        self.slots.iter().position(|slot| slot.item_id == item_id)
    }

    /// Find an empty slot index.
    pub fn find_empty_slot(&self) -> Option<usize> {
        self.slots.iter().position(|slot| slot.item_id == 0)
    }

    /// Set the quantity for an item (creates slot if needed).
    pub fn set_quantity(&mut self, item_id: u32, quantity: u64) -> Result<(), &'static str> {
        if let Some(idx) = self.find_slot(item_id) {
            if quantity == 0 {
                // Clear the slot
                self.slots[idx] = ItemSlot::default();
            } else {
                self.slots[idx].quantity = quantity;
            }
            Ok(())
        } else if quantity > 0 {
            // Need a new slot
            if let Some(idx) = self.find_empty_slot() {
                self.slots[idx] = ItemSlot { item_id, quantity };
                Ok(())
            } else {
                Err("No empty slots available")
            }
        } else {
            // Setting 0 quantity for non-existent item is a no-op
            Ok(())
        }
    }

    /// Withdraw an amount from an item.
    pub fn withdraw(&mut self, item_id: u32, amount: u64) -> Result<(), &'static str> {
        let current = self.get_quantity(item_id);
        if current < amount {
            return Err("Insufficient quantity");
        }
        self.set_quantity(item_id, current - amount)
    }

    /// Deposit an amount to an item.
    pub fn deposit(&mut self, item_id: u32, amount: u64) -> Result<(), &'static str> {
        let current = self.get_quantity(item_id);
        self.set_quantity(item_id, current + amount)
    }
}

/// Circuit variable for an inventory.
pub struct InventoryVar<F: PrimeField> {
    /// Slot variables: (item_id, quantity) pairs
    pub slots: Vec<(FpVar<F>, FpVar<F>)>,
}

impl<F: PrimeField> InventoryVar<F> {
    /// Allocate inventory as witness variables.
    pub fn new_witness(
        cs: ConstraintSystemRef<F>,
        inventory: &Inventory,
    ) -> Result<Self, SynthesisError> {
        let slots = inventory
            .slots
            .iter()
            .map(|slot| {
                let item_id = FpVar::new_witness(cs.clone(), || {
                    Ok(F::from(slot.item_id as u64))
                })?;
                let quantity = FpVar::new_witness(cs.clone(), || {
                    Ok(F::from(slot.quantity))
                })?;
                Ok((item_id, quantity))
            })
            .collect::<Result<Vec<_>, SynthesisError>>()?;

        Ok(Self { slots })
    }

    /// Convert to field element variables for Poseidon hashing.
    pub fn to_field_vars(&self) -> Vec<FpVar<F>> {
        self.slots
            .iter()
            .flat_map(|(id, qty)| vec![id.clone(), qty.clone()])
            .collect()
    }

    /// Get the quantity variable for a specific item_id.
    /// Returns the sum of quantities for all matching slots (should be only one).
    pub fn get_quantity_for_item(
        &self,
        _cs: ConstraintSystemRef<F>,
        target_item_id: &FpVar<F>,
    ) -> Result<FpVar<F>, SynthesisError> {
        let mut total_quantity = FpVar::zero();

        for (slot_id, slot_qty) in &self.slots {
            // Check if this slot matches the target item_id
            let is_match = slot_id.is_eq(target_item_id)?;

            // Conditionally add the quantity if it matches
            let contribution = is_match.select(slot_qty, &FpVar::zero())?;
            total_quantity += contribution;
        }

        Ok(total_quantity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inventory_operations() {
        let mut inv = Inventory::new();

        // Test deposit
        inv.deposit(1, 10).unwrap();
        assert_eq!(inv.get_quantity(1), 10);

        // Test withdraw
        inv.withdraw(1, 3).unwrap();
        assert_eq!(inv.get_quantity(1), 7);

        // Test insufficient balance
        assert!(inv.withdraw(1, 100).is_err());

        // Test from_items
        let inv2 = Inventory::from_items(&[(1, 100), (2, 50)]);
        assert_eq!(inv2.get_quantity(1), 100);
        assert_eq!(inv2.get_quantity(2), 50);
        assert_eq!(inv2.get_quantity(3), 0);
    }

    #[test]
    fn test_to_field_elements() {
        use ark_bn254::Fr;

        let inv = Inventory::from_items(&[(1, 100), (2, 50)]);
        let elements: Vec<Fr> = inv.to_field_elements();

        // Should have 2 elements per slot (id, qty) * 16 slots = 32 elements
        assert_eq!(elements.len(), MAX_ITEM_SLOTS * 2);

        // First slot
        assert_eq!(elements[0], Fr::from(1u64));
        assert_eq!(elements[1], Fr::from(100u64));

        // Second slot
        assert_eq!(elements[2], Fr::from(2u64));
        assert_eq!(elements[3], Fr::from(50u64));
    }
}
