# Q: Why multiple circuits instead of one universal circuit?

## Short Answer

You CAN use one circuit. But there are tradeoffs.

---

## Option A: One Universal Circuit

```rust
struct UniversalInventoryCircuit {
    // All possible inputs
    old_inventory: InventoryData,
    new_inventory: InventoryData,
    old_blinding: Fr,
    new_blinding: Fr,
    item_id: Fr,
    amount: Fr,
    operation_type: Fr,  // 0=exists, 1=withdraw, 2=deposit

    // All possible outputs
    old_commitment: Fr,
    new_commitment: Fr,
}
```

### Inside the Circuit

```rust
fn generate_constraints() {
    // Always verify old commitment
    let computed_old = Poseidon(old_inventory, old_blinding);
    computed_old.enforce_equal(&old_commitment)?;

    // Conditional logic based on operation_type
    if operation_type == 0 {  // ItemExists
        // Only check quantity
        enforce(old_inventory[item_id] >= amount)?;
        // new_commitment unused but still computed
    }
    else if operation_type == 1 {  // Withdraw
        enforce(old_inventory[item_id] >= amount)?;
        let computed_new = Poseidon(new_inventory, new_blinding);
        computed_new.enforce_equal(&new_commitment)?;
    }
    else if operation_type == 2 {  // Deposit
        let computed_new = Poseidon(new_inventory, new_blinding);
        computed_new.enforce_equal(&new_commitment)?;
    }
}
```

---

## The Problem: Circuits Don't Have "If"

**ZK circuits are not programs.** They're constraint systems.

```rust
// This is NOT how circuits work:
if operation_type == 0 {
    // skip these constraints
}

// This IS how circuits work:
// ALL constraints are ALWAYS evaluated
// You use arithmetic tricks to "disable" some
```

### How Conditional Logic Actually Works

```rust
// "If condition, then A == B"
// Becomes: condition * (A - B) == 0

// If condition = 1: A must equal B
// If condition = 0: constraint is trivially satisfied (0 * anything = 0)
```

**But the constraint still exists and costs computation!**

---

## Universal Circuit: The Bloat Problem

```
Universal circuit contains:
  - ItemExists logic: ~300 constraints
  - Withdraw logic: ~450 constraints
  - Deposit logic: ~450 constraints
  - Conditional switching: ~100 constraints
  ─────────────────────────────────────
  Total: ~1300 constraints

Every operation pays for ALL logic, even unused parts.
```

### Performance Impact

| Approach | ItemExists | Withdraw | Deposit |
|----------|------------|----------|---------|
| Separate circuits | 30ms | 45ms | 45ms |
| Universal circuit | 130ms | 130ms | 130ms |

**3-4x slower for simple operations!**

---

## Option B: Separate Specialized Circuits

```
┌─────────────────┐   ┌─────────────────┐   ┌─────────────────┐
│ ItemExistsCircuit│   │ WithdrawCircuit │   │ DepositCircuit  │
│                 │   │                 │   │                 │
│ ~300 constraints│   │ ~450 constraints│   │ ~450 constraints│
│ ~30ms           │   │ ~45ms           │   │ ~45ms           │
└─────────────────┘   └─────────────────┘   └─────────────────┘
```

Each circuit only contains what it needs.

---

## The Tradeoffs

| Aspect | Universal Circuit | Separate Circuits |
|--------|-------------------|-------------------|
| **Proving time** | Slow (all logic) | Fast (only needed logic) |
| **Trusted setups** | 1 | Multiple (1 per circuit) |
| **Verification keys** | 1 (smaller contract) | Multiple (larger contract) |
| **Flexibility** | Add ops = new circuit anyway | Add ops = new circuit |
| **Complexity** | Complex conditional logic | Simple, focused circuits |

---

## The Trusted Setup Cost

This is the main argument FOR universal circuits:

```
Each circuit needs:
  - Trusted setup ceremony (expensive, slow)
  - Verification key stored on-chain (~328 bytes)
  - Proving key stored off-chain (~MB)

3 circuits = 3× the setup cost
```

### But Consider:

1. **Setup is one-time** - Do it once, use forever
2. **VK storage is cheap** - 328 bytes × 3 = ~1KB total
3. **Proving happens often** - Faster proving saves time on every transaction

---

## When Universal Circuit Makes Sense

### 1. Many Similar Operations

```
If you have 20 operation types that differ slightly:
  - Universal circuit: 1 setup
  - Separate circuits: 20 setups (painful!)
```

### 2. Highly Dynamic Logic

```
If operations are determined at runtime:
  - Universal circuit can handle any op
  - Separate circuits need contract to pick the right one
```

### 3. Minimal Operations

```
If you only have 2 operations and they share 90% logic:
  - Overhead of universal is small
  - Might as well combine
```

---

## When Separate Circuits Make Sense (Your Case)

### 1. Few, Distinct Operations

```
ItemExists: Check quantity only
Withdraw: Check + update state
Deposit: Update state only

These are different enough to warrant separation.
```

### 2. Performance Matters

```
Dispensers might be used frequently.
30ms vs 130ms matters for UX.
```

### 3. Simple Contract Logic

```move
public fun withdraw(...) {
    groth16::verify(&WITHDRAW_VK, ...);  // Use withdraw circuit
}

public fun check_item(...) {
    groth16::verify(&ITEM_EXISTS_VK, ...);  // Use exists circuit
}
```

Trivial to route to the right verifier.

---

## Hybrid Approach

You can also combine where it makes sense:

```
┌─────────────────────────┐   ┌─────────────────┐
│ StateTransitionCircuit  │   │ ItemExistsCircuit│
│                         │   │                 │
│ - Withdraw              │   │ - Read-only     │
│ - Deposit               │   │ - No state change│
│ - Transfer              │   │                 │
│                         │   │                 │
│ ~600 constraints        │   │ ~300 constraints│
└─────────────────────────┘   └─────────────────┘

2 circuits instead of 4
State changes share logic
Read-only stays fast
```

---

## Summary

| Question | Answer |
|----------|--------|
| Can we use one circuit? | Yes |
| Should we? | Depends on tradeoffs |
| Main cost of universal | Slower proving (all logic evaluated) |
| Main cost of separate | Multiple trusted setups, multiple VKs |
| For smart inventories | Separate is likely better (few ops, performance matters) |

**Recommendation for your case:** Separate circuits. The operations are distinct, proving speed matters for UX, and 2-3 trusted setups is manageable.

---

*Source: Question from ZK Study Session*
