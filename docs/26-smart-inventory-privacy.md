# Q: How to hide smart inventory contents while keeping programmability?

## The Context

```
┌─────────────────────────────────────────────────────────────────┐
│                     CURRENT SYSTEM                               │
│                                                                  │
│   Off-chain inventories          Smart inventories (on-chain)   │
│   ┌─────────────────┐           ┌─────────────────┐             │
│   │ Player storage  │           │ Dispenser       │             │
│   │ (private, works)│           │ Vending machine │             │
│   │                 │           │ Loot boxes      │             │
│   └─────────────────┘           │ Programmable... │             │
│           │                     └────────┬────────┘             │
│           │                              │                       │
│           │                              ▼                       │
│           │                     PROBLEM: Contents are PUBLIC     │
│           │                     Anyone can see what's inside     │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

**Goal:** Hide smart inventory contents, keep programmability, avoid ZK-ing the entire item flow.

---

## The Insight: You Only Need Hiding, Not Full ZK

Full ZK (like Zcash) solves:
- Hiding ✓
- Trustless origin ✓
- Trustless transfers ✓

You only need:
- **Hiding** ✓

The game server is already trusted for origin. You don't need to prove where items came from on-chain.

---

## Solution: Committed State + Selective Disclosure

### The Pattern

```
On-chain:
  - Store: commitment = Hash(inventory_data, blinding)
  - Contract logic operates on commitments
  - Actual data is hidden

Off-chain:
  - Server knows the plaintext inventory
  - Server provides proofs when needed
  - Game already trusts server anyway
```

### When Is ZK Needed?

Only at the **boundary** where on-chain logic must verify something about the hidden state:

| Operation | Needs ZK? | Why |
|-----------|-----------|-----|
| Store inventory | No | Just store commitment |
| Read inventory (by owner) | No | Owner has plaintext |
| Dispenser: "contains item X?" | **Yes** | Contract needs to verify |
| Withdraw item | **Yes** | Contract needs to verify quantity |
| Deposit item | Maybe | Server could sign the new state |

---

## Design: Minimal ZK Surface

### Data Structure

```move
struct SmartInventory has key {
    commitment: vector<u8>,      // Hash(items, blinding) - HIDDEN
    owner: address,
    dispenser_config: DispenserConfig,  // Public rules
    nonce: u64,
}
```

### Operation 1: Initialize (No ZK)

Server creates inventory, signs the initial commitment:

```
Server signs: (inventory_id, initial_commitment, owner)
Contract stores commitment
```

### Operation 2: Deposit from Game (No ZK)

Items come from trusted off-chain system:

```
1. Player deposits sword from off-chain inventory
2. Server updates plaintext: {sword: 5} → {sword: 6}
3. Server computes new_commitment = Hash(new_inventory, new_blinding)
4. Server signs: (old_commitment, new_commitment, nonce)
5. Contract: verify signature, update commitment
```

No ZK! Server signature is the proof of validity.

### Operation 3: Dispense to Player (Minimal ZK)

Contract needs to verify "inventory has ≥1 sword" without seeing inventory:

```
Server (or player) generates ZK proof:
  Private: inventory_data, blinding
  Public: commitment, item_id, amount

Proof: "commitment opens to data containing ≥1 sword"

Contract:
  1. Verify proof
  2. Accept new_commitment (with sword decremented)
  3. Emit event: "1 sword dispensed to player X"
```

### Operation 4: Read by Owner (No ZK)

Owner already has the plaintext (stored locally or fetched from server).

---

## What This Achieves

```
┌─────────────────────────────────────────────────────────────────┐
│                         PUBLIC VIEW                              │
│                                                                  │
│   Smart Inventory #42:                                           │
│     commitment: 0x7a3f...                                        │
│     owner: 0xAlice                                               │
│     dispenser_config: { price: 10 gold, cooldown: 1 hour }      │
│                                                                  │
│   Observer sees: "Something is in there, but what?"             │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                         OWNER VIEW                               │
│                                                                  │
│   Smart Inventory #42:                                           │
│     contents: { sword: 5, potion: 20, gold: 1000 }              │
│     blinding: 0x9b2c... (secret)                                │
│                                                                  │
│   Owner knows everything                                         │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Handling Programmable Logic

### Case: Price-Based Dispenser

```
Config (public):
  item_type: sword
  price: 10 gold

Dispense logic:
  1. Buyer sends 10 gold
  2. ZK proof: "inventory has ≥1 sword"
  3. Contract: update commitment, send sword to buyer
```

ZK only proves *existence*, not full inventory.

### Case: Conditional Dispenser (e.g., "only if buyer has badge")

```
Config (public):
  requires: badge #7
  dispenses: secret_item

Dispense logic:
  1. Buyer proves they have badge #7 (separate proof or NFT check)
  2. ZK proof: "inventory has ≥1 of item at slot 0"
  3. Item type is HIDDEN (buyer doesn't know what they'll get!)
```

The contract enforces rules without knowing the contents.

### Case: Time-Locked Loot Box

```
Config (public):
  unlock_time: timestamp

Dispense logic:
  1. Check: current_time >= unlock_time
  2. ZK proof: "inventory is non-empty"
  3. Dispense (what? hidden until opened!)
```

---

## ZK Circuits Needed

### Circuit 1: Prove Item Exists

```rust
struct ItemExistsCircuit {
    // Private
    inventory: InventoryData,
    blinding: Fr,

    // Public
    commitment: Fr,
    item_id: u32,
    min_quantity: u64,
}

// Proves: commitment contains ≥min_quantity of item_id
```

~200-500 constraints. Fast.

### Circuit 2: State Transition (Withdraw)

```rust
struct WithdrawCircuit {
    // Private
    old_inventory: InventoryData,
    old_blinding: Fr,
    new_blinding: Fr,

    // Public
    old_commitment: Fr,
    new_commitment: Fr,
    item_id: u32,
    amount: u64,
}

// Proves: new_inventory = old_inventory - amount, commitments correct
```

~500-1000 constraints. Still fast.

### Circuit 3: State Transition (Deposit) - Optional

Could use server signature instead:

```
Server signs: (old_commitment, new_commitment, deposited_items, nonce)
```

No ZK needed if you trust server for deposits.

---

## The Spectrum of Trust

```
Full Trust                                                Zero Trust
(Server signs)                                            (Full ZK)
     │                                                         │
     ▼                                                         ▼
┌─────────┐    ┌─────────────┐    ┌───────────────┐    ┌─────────┐
│ Deposit │    │ Read own    │    │ Dispense      │    │ Transfer│
│ from    │    │ inventory   │    │ to player     │    │ between │
│ game    │    │             │    │               │    │ players │
└─────────┘    └─────────────┘    └───────────────┘    └─────────┘
   Server          No proof         ZK proof for         Full ZK
   signature       needed           withdrawal           (if needed)
```

**You choose where to draw the line.**

---

## Summary: What You Actually Need

| Operation | Mechanism | ZK? |
|-----------|-----------|-----|
| Store inventory | Commitment | No |
| Deposit from game | Server signature | No |
| Owner reads contents | Has plaintext | No |
| Dispense (verify has item) | ZK proof | Yes (small) |
| Withdraw (state transition) | ZK proof | Yes (small) |
| Contract logic on hidden state | ZK proof | Yes (where needed) |

**Minimal ZK surface:** Only when the contract must verify something about hidden data.

---

## Benefits of This Approach

1. **Privacy achieved** - Contents hidden from chain observers
2. **Programmability maintained** - Contract logic still works
3. **No origin ZK needed** - Server handles that off-chain
4. **Small circuits** - Only prove what's needed per operation
5. **Existing system unchanged** - Off-chain inventory works as before

---

*Source: Question from ZK Study Session*
