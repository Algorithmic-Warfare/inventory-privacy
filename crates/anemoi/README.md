# Anemoi

A ZK-friendly hash function implementation for BN254 with R1CS gadgets.

## Overview

Anemoi is a cryptographic hash function designed specifically for zero-knowledge proof systems. It achieves ~2x fewer R1CS constraints compared to Poseidon for equivalent security, making it ideal for Groth16 and other SNARK-based systems.

**Reference:** "New Design Techniques for Efficient Arithmetization-Oriented Hash Functions: Anemoi Permutations and Jive Compression Mode" (CRYPTO 2023)
https://eprint.iacr.org/2022/840

## Features

- **Native implementation** - Fast hashing outside circuits
- **R1CS gadgets** - In-circuit hash verification for arkworks
- **BN254 optimized** - Uses field-specific constants for the BN254 curve
- **Jive compression** - Efficient mode for hashing multiple field elements

## Usage

### Native Hashing

```rust
use anemoi::{anemoi_hash, anemoi_hash_two, anemoi_hash_many};
use ark_bn254::Fr;

// Hash a single element
let h = anemoi_hash(Fr::from(42u64));

// Hash two elements
let h = anemoi_hash_two(Fr::from(1u64), Fr::from(2u64));

// Hash multiple elements
let elements = vec![Fr::from(1u64), Fr::from(2u64), Fr::from(3u64)];
let h = anemoi_hash_many(&elements);
```

### In-Circuit (R1CS Gadgets)

```rust
use anemoi::{anemoi_hash_var, anemoi_hash_two_var, anemoi_hash_many_var};
use ark_r1cs_std::fields::fp::FpVar;
use ark_relations::r1cs::ConstraintSystemRef;

fn my_circuit(cs: ConstraintSystemRef<Fr>, input: FpVar<Fr>) -> FpVar<Fr> {
    // Hash inside the circuit
    anemoi_hash_var(cs, &input).unwrap()
}
```

## Constraint Counts

| Operation | Constraints |
|-----------|-------------|
| `anemoi_hash` (1 element) | ~219 |
| `anemoi_hash_two` (2 elements) | ~219 |
| `anemoi_hash_many` (n elements) | ~219 × ceil(n/2) |

## Security

- 128-bit security level
- Designed for algebraic resistance against:
  - Grobner basis attacks
  - Interpolation attacks
  - Statistical attacks

## License

MIT OR Apache-2.0
