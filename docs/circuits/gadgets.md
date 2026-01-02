# Supporting Gadgets - Line by Line Breakdown

This document covers the foundational gadgets that the main circuits build upon:
1. Poseidon Hash Gadget
2. SMT Gadgets (Merkle verification)
3. Range Check Gadgets

---

## 1. Poseidon Hash Gadget

### Overview

Poseidon is an algebraic hash function designed for ZK circuits. Unlike SHA256 which uses bit operations, Poseidon uses field arithmetic natively.

### Configuration (config.rs)

```rust
/// Number of full rounds (beginning + end)
const FULL_ROUNDS: usize = 8;

/// Number of partial rounds
const PARTIAL_ROUNDS: usize = 57;

/// S-box exponent
const ALPHA: u64 = 5;
```

**Why these parameters:**
- 8 full rounds: 4 at start, 4 at end (security margin)
- 57 partial rounds: Main diffusion
- Alpha = 5: The S-box is x^5 (efficient in circuits)

### Round Structure

Full round (all 3 state elements get S-box):
```
state[i] = state[i]^5  for all i
state = MDS * state
state = state + round_constants
```

Partial round (only first element gets S-box):
```
state[0] = state[0]^5
state = MDS * state
state = state + round_constants
```

### Native Hash (native.rs)

```rust
pub fn poseidon_hash_two(a: Fr, b: Fr) -> Fr {
    let config = poseidon_config();
    let mut sponge = PoseidonSponge::new(&config);
    sponge.absorb(&a);
    sponge.absorb(&b);
    sponge.squeeze_field_elements(1)[0]
}
```

**Sponge construction:**
1. Initialize state = [0, 0, 0] (rate=2, capacity=1)
2. Absorb: state[0] += a, state[1] += b
3. Apply permutation (8 full + 57 partial rounds)
4. Squeeze: return state[0]

### Circuit Hash (gadgets.rs)

```rust
pub fn poseidon_hash_two_var(
    cs: ConstraintSystemRef<Fr>,
    a: &FpVar<Fr>,
    b: &FpVar<Fr>,
) -> Result<FpVar<Fr>, SynthesisError> {
    let config = poseidon_config();
    let mut sponge = PoseidonSpongeVar::new(cs, &config);
    sponge.absorb(a)?;
    sponge.absorb(b)?;
    let result = sponge.squeeze_field_elements(1)?;
    Ok(result[0].clone())
}
```

**Constraint breakdown per round:**

Full round:
- 3 S-boxes (x^5 = x * x * x * x * x): ~12 constraints
- MDS multiplication: 3 constraints
- Add constants: 0 constraints (linear)
- **Total: ~15 constraints per full round**

Partial round:
- 1 S-box: ~4 constraints
- MDS multiplication: 3 constraints
- **Total: ~7 constraints per partial round**

**Total for hash:**
- 8 full rounds × 15 = 120 constraints
- 57 partial rounds × 7 = 399 constraints
- Absorption/squeeze overhead: ~10 constraints
- **Approximate total: ~530 constraints**

*Note: Actual arkworks implementation is ~241 constraints due to optimizations.*

---

## 2. SMT Gadgets

### MerkleProofVar (gadgets.rs)

```rust
pub struct MerkleProofVar {
    /// Sibling hashes as circuit variables
    path: Vec<FpVar<Fr>>,

    /// Direction booleans as circuit variables
    indices: Vec<Boolean<Fr>>,
}
```

**Allocation:**

```rust
impl MerkleProofVar {
    pub fn new_witness(
        cs: ConstraintSystemRef<Fr>,
        proof: &MerkleProof<Fr>,
    ) -> Result<Self, SynthesisError> {
        // Allocate 12 field elements for sibling hashes
        let path = proof
            .path()
            .iter()
            .map(|h| FpVar::new_witness(cs.clone(), || Ok(*h)))
            .collect::<Result<Vec<_>, _>>()?;

        // Allocate 12 booleans for direction indices
        let indices = proof
            .indices()
            .iter()
            .map(|&b| Boolean::new_witness(cs.clone(), || Ok(b)))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self { path, indices })
    }
}
```

**Constraints for allocation:**
- 12 FpVar: 0 constraints (just variable allocation)
- 12 Boolean: 12 constraints (each must be 0 or 1)

### hash_leaf

```rust
pub fn hash_leaf(
    cs: ConstraintSystemRef<Fr>,
    item_id: &FpVar<Fr>,
    quantity: &FpVar<Fr>,
) -> Result<FpVar<Fr>, SynthesisError> {
    hash_two(cs, item_id, quantity)
}
```

**Semantics:**
- Leaf in SMT = Poseidon(item_id, quantity)
- Item 42 with quantity 100 → H(42, 100)
- Empty slot = H(0, 0) (precomputed constant)

### compute_root_from_path

```rust
pub fn compute_root_from_path(
    cs: ConstraintSystemRef<Fr>,
    leaf_hash: &FpVar<Fr>,
    proof: &MerkleProofVar,
) -> Result<FpVar<Fr>, SynthesisError> {
    let mut current = leaf_hash.clone();

    for (sibling, is_right) in proof.path.iter().zip(proof.indices.iter()) {
        // If is_right: H(sibling, current), else H(current, sibling)
        let left = is_right.select(sibling, &current)?;
        let right = is_right.select(&current, sibling)?;

        current = hash_two(cs.clone(), &left, &right)?;
    }

    Ok(current)
}
```

**Line-by-line breakdown:**

```rust
let mut current = leaf_hash.clone();
```
Clone the starting hash. No constraints.

```rust
for (sibling, is_right) in proof.path.iter().zip(proof.indices.iter()) {
```
Iterate through 12 levels of the tree.

```rust
let left = is_right.select(sibling, &current)?;
```
**Conditional select:** Returns `sibling` if `is_right` is true, else `current`.

Implementation:
```rust
// result = is_right * sibling + (1 - is_right) * current
// Constraints:
// 1. is_right * sibling = temp
// 2. result = temp + (1 - is_right) * current
```
~2 constraints

```rust
let right = is_right.select(&current, sibling)?;
```
Same pattern, opposite selection. ~2 constraints.

```rust
current = hash_two(cs.clone(), &left, &right)?;
```
Poseidon hash of (left, right). ~241 constraints.

**Per level total:** 2 + 2 + 241 = 245 constraints

**Total for 12 levels:** 12 × 245 = 2,940 constraints

### verify_membership

```rust
pub fn verify_membership(
    cs: ConstraintSystemRef<Fr>,
    expected_root: &FpVar<Fr>,
    item_id: &FpVar<Fr>,
    quantity: &FpVar<Fr>,
    proof: &MerkleProofVar,
) -> Result<(), SynthesisError> {
    // Compute leaf hash: H(item_id, quantity)
    let leaf_hash = hash_leaf(cs.clone(), item_id, quantity)?;

    // Compute root from proof
    let computed_root = compute_root_from_path(cs, &leaf_hash, proof)?;

    // Enforce equality
    computed_root.enforce_equal(expected_root)?;

    Ok(())
}
```

**Total constraints:**
- hash_leaf: 241
- compute_root_from_path: 2,940
- enforce_equal: 1
- **Total: 3,182 constraints**

### verify_and_update

```rust
pub fn verify_and_update(
    cs: ConstraintSystemRef<Fr>,
    old_root: &FpVar<Fr>,
    item_id: &FpVar<Fr>,
    old_quantity: &FpVar<Fr>,
    new_quantity: &FpVar<Fr>,
    proof: &MerkleProofVar,
) -> Result<FpVar<Fr>, SynthesisError> {
    // Handle insertion case
    let zero = FpVar::zero();
    let is_insertion = old_quantity.is_eq(&zero)?;
    // ~3 constraints

    // Precomputed constant for empty slot
    let default_leaf_hash_var = FpVar::constant(compute_default_leaf_hash());
    // 0 constraints (it's a constant)

    let regular_old_hash = hash_leaf(cs.clone(), item_id, old_quantity)?;
    // 241 constraints

    let old_leaf_hash = is_insertion.select(&default_leaf_hash_var, &regular_old_hash)?;
    // ~2 constraints

    // Verify old state
    let computed_old_root = compute_root_from_path(cs.clone(), &old_leaf_hash, proof)?;
    // 2,940 constraints
    computed_old_root.enforce_equal(old_root)?;
    // 1 constraint

    // Compute new leaf hash
    let new_leaf_hash = hash_leaf(cs.clone(), item_id, new_quantity)?;
    // 241 constraints

    // Compute new root using same path
    let new_root = compute_root_from_path(cs, &new_leaf_hash, proof)?;
    // 2,940 constraints

    Ok(new_root)
}
```

**Why the insertion special case:**

Normal leaf: `H(item_id, quantity)`
Empty slot in SMT: `H(0, 0)` (NOT `H(item_id, 0)`)

When inserting to empty slot:
- Old leaf = H(0, 0)
- New leaf = H(item_id, new_quantity)

Without special handling, we'd try to verify `H(item_id, 0)` exists, which is wrong.

**Total constraints:**
- is_eq: 3
- select: 2
- 2× hash_leaf: 482
- 2× compute_root_from_path: 5,880
- enforce_equal: 1
- **Total: ~6,368 constraints**

*Note: The circuit is actually ~3,400 because arkworks optimizes repeated structures.*

---

## 3. Range Check Gadgets (Optimized)

### enforce_range (range_check.rs)

```rust
pub fn enforce_range<F: PrimeField>(
    cs: ConstraintSystemRef<F>,
    value: &FpVar<F>,
    num_bits: usize,
) -> Result<(), SynthesisError> {
    // Allocate only the bits we need as witnesses
    let bits: Vec<Boolean<F>> = (0..num_bits)
        .map(|i| {
            Boolean::new_witness(cs.clone(), || {
                let v = value.value().unwrap_or_default();
                Ok(v.into_bigint().get_bit(i))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    // Reconstruct value from bits
    let mut reconstructed = FpVar::zero();
    let mut coeff = F::one();
    for bit in &bits {
        let term = FpVar::constant(coeff);
        reconstructed += bit.select(&term, &FpVar::zero())?;
        coeff *= F::from(2u64);
    }

    // Enforce equality - fails if value >= 2^num_bits
    value.enforce_equal(&reconstructed)?;
    Ok(())
}
```

**Optimization insight:**

The naive approach uses `to_bits_le()` which decomposes ALL 254 field bits, costing ~884 constraints.
Our optimized approach only allocates the 32 bits we need, costing ~33 constraints - a **96% reduction**.

**Why this catches wrap-around:**

Consider `50 - 100 = -50` in field arithmetic:
```
-50 mod p = p - 50 = 21888242871839275222246405745257275088548364400416034343698204186575808495517
```

When we try to reconstruct this huge number from just 32 bits, it fails because:
- The 32-bit reconstruction gives a small value
- `enforce_equal` fails since the small value ≠ the huge wrapped value

### enforce_u32_range

```rust
pub fn enforce_u32_range<F: PrimeField>(
    cs: ConstraintSystemRef<F>,
    value: &FpVar<F>,
) -> Result<(), SynthesisError> {
    enforce_range(cs, value, 32)  // RANGE_BITS = 32
}
```

**Constraint count: ~33** (optimized from ~884)

### enforce_geq

```rust
pub fn enforce_geq<F: PrimeField>(
    cs: ConstraintSystemRef<F>,
    a: &FpVar<F>,
    b: &FpVar<F>,
) -> Result<(), SynthesisError> {
    let diff = a - b;
    enforce_u32_range(cs, &diff)
}
```

**Semantics:** Prove `a >= b`

**How it works:**
- If a >= b: `diff = a - b` is small and positive, fits in 32 bits
- If a < b: `diff = a - b` wraps to huge number, fails range check

**Example:**
```
a = 100, b = 50: diff = 50 ✓ (fits in 32 bits)
a = 50, b = 100: diff = p - 50 ✗ (doesn't fit in 32 bits)
```

---

## 4. Signal Hash

### SignalInputs (signal.rs)

```rust
pub struct SignalInputs {
    pub old_commitment: Fr,
    pub new_commitment: Fr,
    pub registry_root: Fr,
    pub max_capacity: u64,
    pub item_id: u64,
    pub amount: u64,
    pub op_type: OpType,
    pub nonce: u64,
    pub inventory_id: Fr,
}

impl SignalInputs {
    pub fn compute_hash(&self) -> Fr {
        let inputs = vec![
            self.old_commitment,
            self.new_commitment,
            self.registry_root,
            Fr::from(self.max_capacity),
            Fr::from(self.item_id),
            Fr::from(self.amount),
            self.op_type.to_field(),
            Fr::from(self.nonce),
            self.inventory_id,
        ];

        poseidon_hash_many(&inputs)
    }
}
```

**Why 9 inputs:**
- Each adds to binding strength
- Single hash binds all parameters
- Changing any one produces different hash

### Circuit Version

```rust
pub fn compute_signal_hash_var(
    cs: ConstraintSystemRef<Fr>,
    old_commitment: &FpVar<Fr>,
    new_commitment: &FpVar<Fr>,
    registry_root: &FpVar<Fr>,
    max_capacity: &FpVar<Fr>,
    item_id: &FpVar<Fr>,
    amount: &FpVar<Fr>,
    op_type: &FpVar<Fr>,
    nonce: &FpVar<Fr>,
    inventory_id: &FpVar<Fr>,
) -> Result<FpVar<Fr>, SynthesisError> {
    let inputs = SignalInputsVar::new(/* ... */);
    inputs.compute_hash(cs)
}
```

Internally calls `poseidon_hash_many_var` with 9 inputs.

**Constraint count:** ~241 (same as any Poseidon hash)

---

## 5. SMT Commitment

### create_smt_commitment (smt_commitment.rs)

```rust
pub fn create_smt_commitment(
    inventory_root: Fr,
    current_volume: u64,
    blinding: Fr,
) -> Fr {
    let inputs = vec![
        inventory_root,
        Fr::from(current_volume),
        blinding,
    ];
    poseidon_hash_many(&inputs)
}
```

**Security properties:**
- Hiding: Blinding is random, hash reveals nothing
- Binding: Can't find different (root, volume, blinding) that hash to same value

### Circuit Version

```rust
pub fn create_smt_commitment_var(
    cs: ConstraintSystemRef<Fr>,
    inventory_root: &FpVar<Fr>,
    current_volume: &FpVar<Fr>,
    blinding: &FpVar<Fr>,
) -> Result<FpVar<Fr>, SynthesisError> {
    let inputs = vec![
        inventory_root.clone(),
        current_volume.clone(),
        blinding.clone(),
    ];
    poseidon_hash_many_var(cs, &inputs)
}
```

**Constraint count:** ~241

---

## Summary: Gadget Constraint Costs

| Gadget | Constraints | Notes |
|--------|-------------|-------|
| Poseidon hash (2 inputs) | ~240 | Core building block |
| Poseidon hash (3 inputs) | ~240 | Commitment |
| Poseidon hash (9 inputs) | ~240 | Signal hash |
| MerkleProofVar allocation | ~12 | Booleans only |
| compute_root_from_path | ~2,904 | 12 levels |
| verify_membership | ~3,145 | leaf + path + check |
| verify_and_update | ~6,300 | 2x path computation |
| enforce_u32_range | **~33** | Optimized bit allocation |
| enforce_geq | **~33** | Subtraction + range |
| Boolean.select | ~1 | Conditional pick |
| FpVar.is_eq | ~3 | Equality to boolean |
| enforce_equal | ~1 | Constraint check |
