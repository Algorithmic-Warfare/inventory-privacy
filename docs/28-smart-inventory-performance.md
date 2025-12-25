# Q: How performant is the simplified smart inventory design?

## Reference: This Codebase's Proximity Circuit

```
ProximityCircuit:
  - Poseidon hash: ~150 constraints
  - Distance calculation: ~50 constraints
  - Total: ~200 constraints
  - Proving time: 15-20ms (release mode)
```

**Rule of thumb:** ~10 constraints ≈ 1ms proving time (Groth16, BN254, modern CPU)

---

## Smart Inventory Circuit Estimates

### Circuit 1: ItemExistsCircuit

"Prove commitment contains ≥N of item X"

```
Constraints breakdown:
  - Commitment verification (Poseidon): ~150
  - Inventory encoding/lookup: ~50-200 (depends on design)
  - Quantity comparison: ~50

Total: ~250-400 constraints
Time: ~25-40ms
```

### Circuit 2: WithdrawCircuit

"Prove old_commitment - items = new_commitment"

```
Constraints breakdown:
  - Old commitment verification (Poseidon): ~150
  - New commitment computation (Poseidon): ~150
  - Item lookup: ~50-200
  - Quantity check (≥ amount): ~50
  - Subtraction: ~10

Total: ~400-600 constraints
Time: ~40-60ms
```

### Circuit 3: DepositCircuit (if needed)

Similar to Withdraw:

```
Total: ~400-600 constraints
Time: ~40-60ms
```

---

## Inventory Encoding Impact

How you encode the inventory affects constraint count significantly:

### Option A: Fixed Slots (Simple)

```rust
struct Inventory {
    slots: [(item_id, quantity); 20],  // 20 fixed slots
}

commitment = Poseidon(slot0, slot1, ..., slot19)
```

```
Constraints:
  - Poseidon with 20 inputs: ~200-250
  - Slot lookup (check each slot): 20 × ~10 = ~200
  - Total overhead: ~400-450
```

### Option B: Sparse/Merkle (Scalable)

```rust
// Items as leaves in a mini Merkle tree
         inventory_root
        /       |       \
    item0    item1    item2
```

```
Constraints:
  - Inventory Merkle proof (depth 5 for 32 items): 5 × 160 = ~800
  - Single item verification: ~50
  - Total overhead: ~850
```

### Option C: Single Item Focus (Minimal)

If you only need to prove ONE item type per operation:

```rust
// Commitment to just (item_id, quantity, other_items_hash)
commitment = Poseidon(item_id, quantity, rest_hash, blinding)
```

```
Constraints:
  - One Poseidon: ~150
  - Quantity check: ~50
  - Total: ~200
```

---

## Complete Performance Table

| Circuit | Constraints | Proving Time | Verification |
|---------|-------------|--------------|--------------|
| ItemExists (simple) | ~300 | ~30ms | <10ms |
| ItemExists (20 slots) | ~500 | ~50ms | <10ms |
| Withdraw (simple) | ~450 | ~45ms | <10ms |
| Withdraw (20 slots) | ~700 | ~70ms | <10ms |
| Withdraw (Merkle, 32 items) | ~1200 | ~120ms | <10ms |

---

## Comparison to Current Proximity Circuit

| Metric | Proximity (current) | SmartInventory (estimated) |
|--------|--------------------|-----------------------------|
| Constraints | ~200 | ~300-700 |
| Proving | 15-20ms | 30-70ms |
| Verification | <10ms | <10ms |
| Proof size | 128 bytes | 128 bytes (same) |

**2-4x more constraints, but still very fast.**

---

## Real-World Usability

### For Game Transactions

```
Operation          Time        Acceptable?
─────────────────────────────────────────
Dispenser use      ~50ms       ✓ Imperceptible
Withdraw item      ~70ms       ✓ Barely noticeable
Batch withdraw     ~150ms      ✓ Small delay
```

### For UI Flow

```
User clicks "Dispense"
  │
  ├─► Generate proof (50ms)     ← Happens while "loading" animation
  │
  ├─► Submit to chain (~1-2s)   ← Network latency dominates
  │
  └─► Confirmation
```

The ZK proving is **not the bottleneck**. Chain confirmation is.

---

## Optimization Opportunities

### 1. Precompute Proofs

If the operation is predictable:
```
User browsing dispenser → precompute proof in background
User clicks "buy" → proof already ready, instant submit
```

### 2. Batch Operations

```
Instead of: 5 separate proofs (5 × 50ms = 250ms)
Do: 1 batch proof for 5 items (~100ms)
```

### 3. Simpler Commitment Scheme

If inventory is small (< 10 item types):
```
commitment = Poseidon(item1_qty, item2_qty, ..., item10_qty, blinding)
```

Direct slots, no lookup logic. ~200-300 constraints total.

---

## On-Chain Verification Cost

Groth16 verification is **constant** regardless of circuit size:

```
Sui gas cost: Fixed for groth16::verify()
Time: <10ms
Proof size: 128 bytes (always)
Public inputs: 32 bytes × number of inputs
```

Larger circuits don't cost more to verify!

---

## Summary

| Aspect | Estimate |
|--------|----------|
| **Simple inventory (10 slots)** | ~300 constraints, ~30ms |
| **Medium inventory (20 slots)** | ~500-700 constraints, ~50-70ms |
| **Large inventory (Merkle)** | ~1000-1500 constraints, ~100-150ms |
| **Verification** | Always <10ms, constant gas |
| **User experience** | Imperceptible to barely noticeable |
| **Bottleneck** | Chain confirmation, not proving |

**Verdict:** Very practical. The simplified design without Merkle trees keeps circuits small and proving fast.

---

*Source: Question from ZK Study Session*
