# Circuit Architecture Documentation

This document provides a comprehensive breakdown of every ZK circuit in the inventory-privacy system, explaining each constraint and the reasoning behind the design.

## Overview

The system uses three main circuits built on the Groth16 proving system with the BN254 curve:

| Circuit | Purpose | Constraints | Proof Time |
|---------|---------|-------------|------------|
| **StateTransition** | Deposit/Withdraw with capacity | ~8,597 | ~450ms |
| **ItemExists** | Prove ownership >= N items | ~4,124 | ~200ms |
| **Capacity** | Prove volume <= max_capacity | ~724 | ~40ms |

## Table of Contents

1. [Foundational Concepts](#foundational-concepts)
2. [StateTransition Circuit](./state_transition.md)
3. [ItemExists Circuit](./item_exists.md)
4. [Capacity Circuit](./capacity.md)
5. [Supporting Gadgets](./gadgets.md)

---

## Foundational Concepts

### What is an R1CS Circuit?

R1CS (Rank-1 Constraint System) is how we express computations that can be proven in zero-knowledge. Every computation is broken down into constraints of the form:

```
A * B = C
```

Where A, B, C are linear combinations of variables. The prover must find values for all variables that satisfy every constraint simultaneously.

### Public Inputs vs Witnesses

- **Public Inputs**: Values visible to both prover and verifier. These are committed to in the proof.
- **Witnesses**: Private values known only to the prover. The proof demonstrates the prover knows valid witnesses without revealing them.

### The Signal Hash Pattern

Sui has a limit of 8 public inputs for ZK proofs. We use a "signal hash" pattern to compress many parameters into a single hash:

```
signal_hash = Poseidon(
    old_commitment,    // Previous inventory state
    new_commitment,    // New inventory state
    registry_root,     // Volume registry commitment
    max_capacity,      // Capacity limit
    item_id,           // Item being operated on
    amount,            // Quantity change
    op_type,           // 0=deposit, 1=withdraw
    nonce,             // Replay protection
    inventory_id       // Cross-inventory protection
)
```

The verifier checks that the proof's signal_hash matches the on-chain computed hash, binding all parameters.

### SMT Commitment Scheme

Each inventory's state is committed as:

```
commitment = Poseidon(inventory_root, current_volume, blinding)
```

Where:
- `inventory_root`: Root hash of the Sparse Merkle Tree containing all items
- `current_volume`: Total volume of all items (tracked incrementally)
- `blinding`: Random value that hides the commitment (prevents rainbow table attacks)

---

## Why These Design Choices?

### 1. Sparse Merkle Tree (Depth 12)

**Why SMT over regular Merkle tree?**
- Game inventories are sparse (few items out of thousands possible)
- SMT allows efficient proofs for non-existent items (empty slots)
- O(log n) proof size regardless of which items exist

**Why depth 12?**
- Supports 2^12 = 4,096 item types
- Each level adds ~241 constraints (one Poseidon hash)
- Depth 12 balances item capacity vs constraint count

### 2. Poseidon Hash Function

**Why Poseidon over SHA256/Keccak?**
- Poseidon is designed for arithmetic circuits (R1CS-friendly)
- ~241 constraints per hash vs ~25,000 for SHA256
- Still cryptographically secure (128-bit security)

**Why we switched from Anemoi:**
- Anemoi has fewer constraints (~126) but expensive witness generation
- The x^(1/5) inverse S-box requires computing a 254-bit exponent
- Net result: Poseidon proves ~2x faster despite 2x more constraints

### 3. Optimized Range Checks

**Why range checks at all?**
- Field arithmetic wraps around (no overflow/underflow)
- Without checks, `5 - 10` becomes a huge positive number
- Range checks ensure values stay in expected bounds

**Optimized implementation:**
- Only allocates 32 bits as witnesses, reconstructs and verifies
- ~33 constraints per range check (vs ~884 for naive bit decomposition)
- 32 bits supports ~4.29 billion - more than any game needs

### 4. Volume Tracking

**Why track volume incrementally?**
- Alternative: Sum all items in every proof (expensive)
- With incremental tracking: just verify delta each operation
- Volume is committed in the state, so it can't be cheated

---

## Constraint Breakdown Summary

### StateTransition (~8,597 constraints)

| Component | Constraints | Purpose |
|-----------|-------------|---------|
| SMT verify_and_update | ~6,300 | Verify old root, compute new root |
| 2x commitment hashes | ~482 | Create old/new commitments |
| Signal hash | ~241 | Bind all parameters |
| 3x optimized range checks | ~99 | Prevent underflow (qty, vol, capacity) |
| Quantity/volume logic | ~50 | Enforce correct arithmetic |
| Boolean operations | ~20 | Operation type handling |

### ItemExists (~4,124 constraints)

| Component | Constraints | Purpose |
|-----------|-------------|---------|
| SMT verify_membership | ~3,133 | Prove item exists with quantity |
| Commitment hash | ~241 | Compute commitment |
| Public hash | ~241 | Bind commitment + item + min_qty |
| Variable allocation | ~10 | Witness setup |

### Capacity (~724 constraints)

| Component | Constraints | Purpose |
|-----------|-------------|---------|
| Commitment hash | ~241 | Compute commitment |
| Public hash | ~241 | Bind commitment + max_capacity |
| Variable allocation | ~5 | Witness setup |

---

## Security Properties

### 1. Soundness
An invalid proof will be rejected with overwhelming probability (2^-128).

### 2. Completeness
A prover with valid witnesses can always generate an accepting proof.

### 3. Zero-Knowledge
The proof reveals nothing about witnesses beyond what's derivable from public inputs.

### 4. Attack Prevention

| Attack | Prevention |
|--------|------------|
| Replay | Nonce in signal hash, verified on-chain |
| Cross-inventory | Inventory ID in signal hash |
| Underflow | 32-bit range checks on quantities |
| Overflow | 32-bit range checks on volumes |
| Capacity bypass | Explicit capacity check |
| State manipulation | Merkle proof verification |

---

## Further Reading

- [StateTransition Circuit Deep Dive](./state_transition.md)
- [ItemExists Circuit Deep Dive](./item_exists.md)
- [Capacity Circuit Deep Dive](./capacity.md)
- [Supporting Gadgets](./gadgets.md)
