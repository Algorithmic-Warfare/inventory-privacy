# Q: Can we have unbounded item slots with volume-based limits?

## The Requirement

```
Instead of: 16 fixed slots
Want: Any items, limited by total volume

Each item has a V-size (volume).
Inventory has max_volume capacity.
Sum of (quantity × item_volume) ≤ max_volume
```

---

## The Challenge: Circuits Are Fixed Size

ZK circuits must have a fixed number of constraints. You can't have "for each item in unbounded list."

**But we can simulate unbounded with clever encoding.**

---

## Solution: Sparse Merkle Tree of Items

### The Structure

```
                    inventory_root
                   /              \
                 ...              ...
                /                    \
        item_id: 1              item_id: 42
        quantity: 5             quantity: 100
        (empty slots = default value)
```

Each leaf is:
```
leaf = Poseidon(item_id, quantity)
```

Empty slots have a known default: `Poseidon(0, 0)`

### Properties

- Tree depth = 32 (supports 2^32 unique item types)
- Only non-empty items matter
- Proving involves just the items you're operating on
- Total volume tracked separately

---

## Data Structures

### On-Chain State

```move
struct VolumeInventory has key {
    id: UID,
    items_root: vector<u8>,      // Merkle root of all items
    total_volume: u64,           // Current volume used
    max_volume: u64,             // Capacity
    owner: address,
    nonce: u64,
    blinding_commitment: vector<u8>,  // Hide the blinding factor
}
```

### Off-Chain State (User's Device)

```rust
struct InventoryState {
    items: HashMap<u32, u64>,    // item_id → quantity (sparse!)
    total_volume: u64,
    blinding: Fr,
    merkle_tree: SparseMerkleTree,
}
```

### Item Registry (Separate, Public)

```move
struct ItemRegistry has key {
    volumes: Table<u32, u64>,    // item_id → volume per unit
}

// Example:
// sword (id: 1) → volume: 10
// potion (id: 2) → volume: 2
// gold (id: 3) → volume: 1
```

---

## Commitment Scheme

```
items_root = SparseMerkleRoot(all item leaves)
inventory_commitment = Poseidon(items_root, total_volume, blinding)
```

This commits to:
- All items (via Merkle root)
- Total volume used
- Hidden by blinding

---

## Circuits

### Circuit 1: ItemExistsCircuit (Volume-Based)

```rust
struct ItemExistsCircuit {
    // Private
    item_quantity: u64,
    merkle_path: Vec<Fr>,        // Path for this item
    merkle_indices: Vec<bool>,
    blinding: Fr,
    total_volume: u64,

    // Public
    items_root: Fr,
    inventory_commitment: Fr,
    item_id: u32,
    min_quantity: u64,
}

// Proves:
// 1. Merkle path valid: leaf(item_id, item_quantity) in tree with items_root
// 2. Commitment valid: Poseidon(items_root, total_volume, blinding) = inventory_commitment
// 3. item_quantity >= min_quantity
```

**Constraints:** ~3500 (32-level Merkle + Poseidon + comparison)

### Circuit 2: WithdrawCircuit (Volume-Based)

```rust
struct WithdrawCircuit {
    // Private
    old_quantity: u64,
    new_quantity: u64,           // old_quantity - amount
    old_merkle_path: Vec<Fr>,
    new_items_root: Fr,          // After update
    old_blinding: Fr,
    new_blinding: Fr,
    old_total_volume: u64,
    new_total_volume: u64,
    item_volume: u64,            // Volume per unit of this item

    // Public
    old_inventory_commitment: Fr,
    new_inventory_commitment: Fr,
    item_id: u32,
    amount: u64,
}

// Proves:
// 1. Old leaf valid in old tree
// 2. new_quantity = old_quantity - amount
// 3. new_total_volume = old_total_volume - (amount × item_volume)
// 4. New tree root correct after updating leaf
// 5. Both commitments valid
```

**Constraints:** ~7000 (two Merkle proofs + updates + arithmetic)

### Circuit 3: DepositCircuit (Volume-Based)

```rust
struct DepositCircuit {
    // Similar to Withdraw, but:
    // - new_quantity = old_quantity + amount
    // - new_total_volume = old_total_volume + (amount × item_volume)
    // - Enforce: new_total_volume <= max_volume

    // Additional public input
    max_volume: u64,
}

// Extra constraint:
// new_total_volume <= max_volume (volume capacity check)
```

### Circuit 4: TransferCircuit

```rust
// Combines withdraw from src + deposit to dst
// Constraints: ~14000
```

---

## Volume Validation

### Option A: Volume in Circuit (Private Registry)

```rust
// item_volume is private input, trusted from user
// Risk: User could lie about item volumes
```

**Not recommended** - user could claim sword has volume 0.

### Option B: Volume from Public Registry (Recommended)

```rust
// item_volume is public input
// Contract checks: volume == registry.volumes[item_id]

struct DepositCircuit {
    // Public inputs include item_volume
    item_volume: u64,  // Public!
}
```

```move
public fun deposit(..., item_id: u32, item_volume: u64, ...) {
    // Verify volume matches registry
    assert!(
        item_volume == table::borrow(&registry.volumes, item_id),
        EInvalidVolume
    );

    // Then verify ZK proof (which uses this volume)
    groth16::verify(...);
}
```

**Volume is public, verified against registry, then used in proof.**

### Option C: Volume Merkle Tree (Private but Verified)

If you want to hide which item type:

```
Volume registry as Merkle tree (public root)
Circuit proves: "item_id has volume V" via Merkle proof
```

Adds ~3500 constraints but keeps item_id private.

---

## Performance Estimates

| Circuit | Constraints | Proving Time |
|---------|-------------|--------------|
| ItemExists | ~3,500 | ~350ms |
| Withdraw | ~7,000 | ~700ms |
| Deposit | ~7,500 | ~750ms |
| Transfer | ~14,000 | ~1.4s |

**Slower than fixed slots, but still practical.**

### Optimization: Smaller Merkle Tree

If you limit to 1024 unique item types (depth 10):

| Circuit | Constraints | Proving Time |
|---------|-------------|--------------|
| ItemExists | ~1,800 | ~180ms |
| Withdraw | ~3,500 | ~350ms |
| Deposit | ~4,000 | ~400ms |
| Transfer | ~7,000 | ~700ms |

**Much more practical!**

---

## Hybrid Approach: Best of Both

```rust
struct Inventory {
    // Common items in fixed slots (fast)
    common_slots: [(u32, u64); 8],   // 8 most-used items

    // Rare items in Merkle tree (flexible)
    rare_items_root: Fr,

    total_volume: u64,
    blinding: Fr,
}
```

- Swords, potions, gold → fixed slots (~500 constraints)
- Rare drops → Merkle tree (~3000 constraints when accessed)

Most operations hit the fast path!

---

## Contract Design

```move
struct VolumeInventory has key {
    id: UID,

    // Commitment to full state
    commitment: vector<u8>,

    // Public volume tracking (optional, for UX)
    total_volume: u64,      // Can be public
    max_volume: u64,        // Capacity

    owner: address,
    nonce: u64,
}

public fun deposit(
    inventory: &mut VolumeInventory,
    registry: &ItemRegistry,
    vks: &VerifyingKeys,
    proof: vector<u8>,
    new_commitment: vector<u8>,
    new_total_volume: u64,
    item_id: u32,
    amount: u64,
) {
    // Get item volume from public registry
    let item_volume = *table::borrow(&registry.volumes, item_id);

    // Build public inputs
    let public_inputs = vector[
        inventory.commitment,
        new_commitment,
        item_id,
        amount,
        item_volume,
        inventory.total_volume,
        new_total_volume,
        inventory.max_volume,
    ];

    // Verify proof
    let pvk = groth16::prepare_verifying_key(&vks.curve, &vks.deposit_vk);
    assert!(groth16::verify_groth16_proof(&vks.curve, &pvk, &public_inputs, &proof));

    // Verify volume didn't exceed max (redundant with circuit, but defense in depth)
    assert!(new_total_volume <= inventory.max_volume, EVolumeExceeded);

    // Update
    inventory.commitment = new_commitment;
    inventory.total_volume = new_total_volume;
    inventory.nonce = inventory.nonce + 1;
}
```

---

## Summary

| Approach | Items | Constraints | Proving Time | Flexibility |
|----------|-------|-------------|--------------|-------------|
| Fixed slots (16) | 16 types | ~600 | ~60ms | Low |
| Sparse Merkle (depth 32) | 4B types | ~7000 | ~700ms | High |
| Sparse Merkle (depth 10) | 1024 types | ~3500 | ~350ms | Medium |
| Hybrid (8 slots + Merkle) | 8 fast + unlimited | ~500-3500 | 50-350ms | High |

**Recommendation:** Hybrid approach for production. Sparse Merkle (depth 10) for PoC simplicity.

---

## Key Changes from Fixed Slots

| Aspect | Fixed Slots | Volume-Based |
|--------|-------------|--------------|
| Item limit | N slots | Volume capacity |
| Data structure | Array | Sparse Merkle Tree |
| Commitment | Poseidon(slots...) | Poseidon(merkle_root, volume, blinding) |
| Per-item proof | O(1) | O(log n) Merkle path |
| Adding new item type | Must fit in slot | Just add leaf |
| Volume tracking | N/A | Explicit total_volume |

---

*Source: Question from ZK Study Session*
