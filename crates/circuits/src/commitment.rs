//! Poseidon commitment scheme for inventories.

use ark_crypto_primitives::sponge::{
    poseidon::{PoseidonConfig, PoseidonSponge},
    Absorb, CryptographicSponge,
};
use ark_ff::PrimeField;
use ark_r1cs_std::fields::fp::FpVar;
use ark_relations::r1cs::{ConstraintSystemRef, SynthesisError};

use crate::inventory::Inventory;

/// Generate Poseidon configuration for BN254.
/// Uses standard parameters suitable for ZK circuits.
pub fn poseidon_config<F: PrimeField + Absorb>() -> PoseidonConfig<F> {
    // Standard Poseidon parameters for BN254
    // Rate: 2, Capacity: 1, Full rounds: 8, Partial rounds: 57
    let full_rounds = 8;
    let partial_rounds = 57;
    let alpha = 5;
    let rate = 2;

    // Generate round constants and MDS matrix
    // In production, these should come from a trusted source
    let (ark, mds) = generate_poseidon_parameters::<F>(rate, full_rounds, partial_rounds);

    PoseidonConfig::new(
        full_rounds,
        partial_rounds,
        alpha,
        mds,
        ark,
        rate,
        1, // capacity
    )
}

/// Generate Poseidon parameters (simplified for PoC).
/// In production, use parameters from a trusted ceremony.
fn generate_poseidon_parameters<F: PrimeField>(
    rate: usize,
    full_rounds: usize,
    partial_rounds: usize,
) -> (Vec<Vec<F>>, Vec<Vec<F>>) {
    let width = rate + 1;
    let total_rounds = full_rounds + partial_rounds;

    // Generate deterministic round constants
    let mut ark = Vec::with_capacity(total_rounds);
    for round in 0..total_rounds {
        let mut round_constants = Vec::with_capacity(width);
        for i in 0..width {
            // Simple deterministic generation (NOT cryptographically secure)
            // In production, use proper parameter generation
            let seed = ((round * width + i + 1) as u64).wrapping_mul(0x9e3779b97f4a7c15);
            round_constants.push(F::from(seed));
        }
        ark.push(round_constants);
    }

    // Generate MDS matrix (circulant construction)
    let mut mds = Vec::with_capacity(width);
    for i in 0..width {
        let mut row = Vec::with_capacity(width);
        for j in 0..width {
            // Simple MDS construction
            let val = if i == j {
                F::from(2u64)
            } else {
                F::from(1u64)
            };
            row.push(val);
        }
        mds.push(row);
    }

    (ark, mds)
}

/// Create a Poseidon commitment to an inventory.
/// commitment = Poseidon(slot0_id, slot0_qty, slot1_id, slot1_qty, ..., blinding)
pub fn create_inventory_commitment<F: PrimeField + Absorb>(
    inventory: &Inventory,
    blinding: F,
    config: &PoseidonConfig<F>,
) -> F {
    let mut inputs = inventory.to_field_elements();
    inputs.push(blinding);

    let mut sponge = PoseidonSponge::new(config);
    sponge.absorb(&inputs);
    sponge.squeeze_field_elements(1)[0]
}

/// Poseidon gadget for in-circuit commitment computation.
pub struct PoseidonGadget<F: PrimeField> {
    config: PoseidonConfig<F>,
}

impl<F: PrimeField + Absorb> PoseidonGadget<F> {
    pub fn new(config: PoseidonConfig<F>) -> Self {
        Self { config }
    }

    /// Compute Poseidon hash in-circuit.
    pub fn hash(
        &self,
        _cs: ConstraintSystemRef<F>,
        inputs: &[FpVar<F>],
    ) -> Result<FpVar<F>, SynthesisError> {
        use ark_crypto_primitives::sponge::constraints::CryptographicSpongeVar;
        use ark_crypto_primitives::sponge::poseidon::constraints::PoseidonSpongeVar;

        let mut sponge = PoseidonSpongeVar::new(_cs.clone(), &self.config);
        sponge.absorb(&inputs)?;
        let output = sponge.squeeze_field_elements(1)?;
        Ok(output[0].clone())
    }

    /// Compute inventory commitment in-circuit.
    pub fn commit_inventory(
        &self,
        cs: ConstraintSystemRef<F>,
        inventory_vars: &[FpVar<F>],
        blinding: &FpVar<F>,
    ) -> Result<FpVar<F>, SynthesisError> {
        let mut inputs = inventory_vars.to_vec();
        inputs.push(blinding.clone());
        self.hash(cs, &inputs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_bn254::Fr;
    use ark_r1cs_std::alloc::AllocVar;
    use ark_r1cs_std::eq::EqGadget;
    use ark_relations::r1cs::ConstraintSystem;

    #[test]
    fn test_commitment_deterministic() {
        let config = poseidon_config::<Fr>();
        let inv = Inventory::from_items(&[(1, 100), (2, 50)]);
        let blinding = Fr::from(12345u64);

        let commitment1 = create_inventory_commitment(&inv, blinding, &config);
        let commitment2 = create_inventory_commitment(&inv, blinding, &config);

        assert_eq!(commitment1, commitment2);
    }

    #[test]
    fn test_different_blinding_different_commitment() {
        let config = poseidon_config::<Fr>();
        let inv = Inventory::from_items(&[(1, 100)]);

        let commitment1 = create_inventory_commitment(&inv, Fr::from(1u64), &config);
        let commitment2 = create_inventory_commitment(&inv, Fr::from(2u64), &config);

        assert_ne!(commitment1, commitment2);
    }

    #[test]
    fn test_in_circuit_commitment() {
        let config = poseidon_config::<Fr>();
        let inv = Inventory::from_items(&[(1, 100)]);
        let blinding = Fr::from(12345u64);

        // Compute out-of-circuit
        let expected = create_inventory_commitment(&inv, blinding, &config);

        // Compute in-circuit
        let cs = ConstraintSystem::<Fr>::new_ref();
        let gadget = PoseidonGadget::new(config);

        let field_elements: Vec<Fr> = inv.to_field_elements();
        let inv_vars: Vec<FpVar<Fr>> = field_elements
            .iter()
            .map(|f| FpVar::new_witness(cs.clone(), || Ok(*f)).unwrap())
            .collect();
        let blinding_var = FpVar::new_witness(cs.clone(), || Ok(blinding)).unwrap();

        let computed = gadget
            .commit_inventory(cs.clone(), &inv_vars, &blinding_var)
            .unwrap();

        // Verify they match
        let expected_var = FpVar::new_input(cs.clone(), || Ok(expected)).unwrap();
        computed.enforce_equal(&expected_var).unwrap();

        assert!(cs.is_satisfied().unwrap());
    }
}
