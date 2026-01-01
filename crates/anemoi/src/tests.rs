//! Integration tests for Anemoi hash function.

use super::native::anemoi_hash_two;
use super::gadgets::anemoi_hash_two_var;
use ark_bn254::Fr;
use ark_ff::{BigInteger, PrimeField};
use ark_r1cs_std::prelude::*;
use ark_r1cs_std::fields::fp::FpVar;
use ark_relations::r1cs::ConstraintSystem;

#[test]
fn test_anemoi_basic() {
    let h = anemoi_hash_two(Fr::from(1u64), Fr::from(2u64));
    assert_ne!(h, Fr::from(0u64));
}

#[test]
fn test_anemoi_circuit_consistency() {
    // Test multiple input pairs
    let test_cases = [
        (0u64, 0u64),
        (1u64, 0u64),
        (0u64, 1u64),
        (1u64, 2u64),
        (100u64, 200u64),
        (u64::MAX, u64::MAX),
    ];

    for (a, b) in test_cases {
        let a_fr = Fr::from(a);
        let b_fr = Fr::from(b);

        // Native
        let native = anemoi_hash_two(a_fr, b_fr);

        // Circuit
        let cs = ConstraintSystem::<Fr>::new_ref();
        let a_var = FpVar::new_witness(cs.clone(), || Ok(a_fr)).unwrap();
        let b_var = FpVar::new_witness(cs.clone(), || Ok(b_fr)).unwrap();
        let h_var = anemoi_hash_two_var(cs.clone(), &a_var, &b_var).unwrap();

        assert!(
            cs.is_satisfied().unwrap(),
            "Circuit unsatisfied for ({}, {})",
            a,
            b
        );
        assert_eq!(
            h_var.value().unwrap(),
            native,
            "Mismatch for ({}, {})",
            a,
            b
        );
    }
}

#[test]
fn test_anemoi_collision_resistance() {
    // Verify no obvious collisions
    let mut hashes = std::collections::HashSet::new();

    for i in 0u64..100 {
        for j in 0u64..10 {
            let h = anemoi_hash_two(Fr::from(i), Fr::from(j));
            let h_bytes: Vec<u8> = h.into_bigint().to_bytes_le();
            assert!(
                hashes.insert(h_bytes),
                "Collision detected at ({}, {})",
                i,
                j
            );
        }
    }
}

#[test]
fn test_anemoi_constraint_count() {
    // Measure Anemoi constraints for 2:1 hash
    let cs = ConstraintSystem::<Fr>::new_ref();
    let a = FpVar::new_witness(cs.clone(), || Ok(Fr::from(1u64))).unwrap();
    let b = FpVar::new_witness(cs.clone(), || Ok(Fr::from(2u64))).unwrap();
    let _ = anemoi_hash_two_var(cs.clone(), &a, &b).unwrap();
    let constraints = cs.num_constraints();

    println!("Anemoi 2:1 hash constraints: {}", constraints);

    // Anemoi should have < 200 constraints for 2:1 hash
    // With 21 rounds and optimized S-box witnessing
    assert!(
        constraints < 200,
        "Expected < 200 constraints, got {}",
        constraints
    );
}
