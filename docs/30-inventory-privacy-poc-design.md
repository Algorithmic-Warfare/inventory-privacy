# Inventory Privacy PoC - Architecture Design

## Overview

A proof-of-concept demonstrating hidden on-chain inventory state with verifiable operations.

```
┌─────────────────────────────────────────────────────────────────┐
│                     INVENTORY-PRIVACY POC                        │
│                                                                  │
│  Similar to location-privacy, but for inventory data:            │
│                                                                  │
│  location-privacy:                                               │
│    commitment = Poseidon(x, y, z, blinding)                     │
│    proves: "I'm within distance D"                              │
│                                                                  │
│  inventory-privacy:                                              │
│    commitment = Poseidon(inventory_data, blinding)              │
│    proves: "I have ≥N of item X" / "state transition valid"     │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Repository Structure

```
inventory-privacy/
├── crates/
│   ├── circuits/                    # ZK circuit definitions
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── inventory.rs         # Inventory data structures
│   │   │   ├── commitment.rs        # Poseidon commitment logic
│   │   │   ├── item_exists.rs       # ItemExistsCircuit
│   │   │   ├── withdraw.rs          # WithdrawCircuit
│   │   │   ├── deposit.rs           # DepositCircuit
│   │   │   ├── transfer.rs          # TransferCircuit (inventory→inventory)
│   │   │   └── tests.rs
│   │   └── Cargo.toml
│   │
│   ├── prover/                      # Proof generation library
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── setup.rs             # Trusted setup utilities
│   │   │   ├── prove.rs             # Proof generation
│   │   │   └── verify.rs            # Local verification (for testing)
│   │   └── Cargo.toml
│   │
│   └── proof-server/                # HTTP API for proof generation
│       ├── src/
│       │   ├── main.rs
│       │   ├── routes.rs
│       │   └── handlers.rs
│       └── Cargo.toml
│
├── packages/
│   └── inventory/                   # Sui Move contracts
│       ├── sources/
│       │   ├── inventory.move       # Core inventory struct + verification
│       │   ├── registry.move        # Inventory registry/factory
│       │   └── events.move          # Event definitions
│       ├── tests/
│       │   └── inventory_tests.move
│       └── Move.toml
│
├── scripts/
│   ├── setup.sh                     # Generate proving/verifying keys
│   ├── test.sh                      # Run all tests
│   └── e2e-test.sh                  # Full end-to-end test
│
├── keys/                            # Generated keys (gitignored)
│   ├── item_exists.pk
│   ├── item_exists.vk
│   ├── withdraw.pk
│   ├── withdraw.vk
│   ├── deposit.pk
│   ├── deposit.vk
│   ├── transfer.pk
│   └── transfer.vk
│
└── README.md
```

---

## Data Structures

### Inventory Encoding

```rust
// crates/circuits/src/inventory.rs

/// Fixed-slot inventory (simple, efficient)
pub const MAX_ITEM_SLOTS: usize = 16;

#[derive(Clone)]
pub struct Inventory {
    /// Slots: (item_id, quantity) pairs
    /// item_id = 0 means empty slot
    pub slots: [(u32, u64); MAX_ITEM_SLOTS],
}

impl Inventory {
    pub fn to_field_elements(&self) -> Vec<Fr> {
        // Flatten to field elements for Poseidon
        self.slots.iter()
            .flat_map(|(id, qty)| vec![Fr::from(*id), Fr::from(*qty)])
            .collect()
    }

    pub fn get_quantity(&self, item_id: u32) -> u64 {
        self.slots.iter()
            .find(|(id, _)| *id == item_id)
            .map(|(_, qty)| *qty)
            .unwrap_or(0)
    }
}
```

### Commitment Scheme

```rust
// crates/circuits/src/commitment.rs

/// commitment = Poseidon(slot0_id, slot0_qty, slot1_id, slot1_qty, ..., blinding)
pub fn create_inventory_commitment(
    inventory: &Inventory,
    blinding: Fr,
    config: &PoseidonConfig<Fr>,
) -> Fr {
    let mut inputs = inventory.to_field_elements();
    inputs.push(blinding);

    let mut sponge = PoseidonSponge::new(config);
    sponge.absorb(&inputs);
    sponge.squeeze_field_elements(1)[0]
}
```

---

## Circuits

### Circuit 1: ItemExistsCircuit

```rust
// crates/circuits/src/item_exists.rs

/// Proves: "Commitment contains ≥ min_quantity of item_id"
pub struct ItemExistsCircuit {
    // Private witnesses
    pub inventory: Option<Inventory>,
    pub blinding: Option<Fr>,

    // Public inputs
    pub commitment: Option<Fr>,
    pub item_id: u32,
    pub min_quantity: u64,

    // Config
    pub poseidon_config: Arc<PoseidonConfig<Fr>>,
}

impl ConstraintSynthesizer<Fr> for ItemExistsCircuit {
    fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
        // 1. Allocate witnesses
        // 2. Allocate public inputs
        // 3. Verify: Poseidon(inventory, blinding) == commitment
        // 4. Verify: inventory.get(item_id) >= min_quantity
    }
}
```

### Circuit 2: WithdrawCircuit

```rust
// crates/circuits/src/withdraw.rs

/// Proves: "old_inventory - amount = new_inventory, commitments valid"
pub struct WithdrawCircuit {
    // Private witnesses
    pub old_inventory: Option<Inventory>,
    pub new_inventory: Option<Inventory>,
    pub old_blinding: Option<Fr>,
    pub new_blinding: Option<Fr>,

    // Public inputs
    pub old_commitment: Option<Fr>,
    pub new_commitment: Option<Fr>,
    pub item_id: u32,
    pub amount: u64,

    pub poseidon_config: Arc<PoseidonConfig<Fr>>,
}

impl ConstraintSynthesizer<Fr> for WithdrawCircuit {
    fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
        // 1. Verify old_commitment = Poseidon(old_inventory, old_blinding)
        // 2. Verify new_commitment = Poseidon(new_inventory, new_blinding)
        // 3. Verify old_inventory[item_id] >= amount
        // 4. Verify new_inventory[item_id] = old_inventory[item_id] - amount
        // 5. Verify all other slots unchanged
    }
}
```

### Circuit 3: DepositCircuit

```rust
// crates/circuits/src/deposit.rs

/// Proves: "old_inventory + amount = new_inventory, commitments valid"
pub struct DepositCircuit {
    // Similar to WithdrawCircuit but addition instead of subtraction
}
```

### Circuit 4: TransferCircuit

```rust
// crates/circuits/src/transfer.rs

/// Proves: "src_inventory -= amount, dst_inventory += amount, both valid"
pub struct TransferCircuit {
    // Private witnesses
    pub src_old_inventory: Option<Inventory>,
    pub src_new_inventory: Option<Inventory>,
    pub src_old_blinding: Option<Fr>,
    pub src_new_blinding: Option<Fr>,

    pub dst_old_inventory: Option<Inventory>,
    pub dst_new_inventory: Option<Inventory>,
    pub dst_old_blinding: Option<Fr>,
    pub dst_new_blinding: Option<Fr>,

    // Public inputs
    pub src_old_commitment: Option<Fr>,
    pub src_new_commitment: Option<Fr>,
    pub dst_old_commitment: Option<Fr>,
    pub dst_new_commitment: Option<Fr>,
    pub item_id: u32,
    pub amount: u64,

    pub poseidon_config: Arc<PoseidonConfig<Fr>>,
}
```

---

## Move Contracts

### Core Inventory

```move
// packages/inventory/sources/inventory.move

module inventory::inventory {
    use sui::groth16;

    /// A private inventory with hidden contents
    struct PrivateInventory has key, store {
        id: UID,
        commitment: vector<u8>,  // 32 bytes - Poseidon hash
        owner: address,
        nonce: u64,
    }

    /// Verification keys for each circuit
    struct VerifyingKeys has key {
        id: UID,
        item_exists_vk: vector<u8>,
        withdraw_vk: vector<u8>,
        deposit_vk: vector<u8>,
        transfer_vk: vector<u8>,
        curve: groth16::Curve,
    }

    /// Create a new private inventory with initial commitment
    public fun create(
        initial_commitment: vector<u8>,
        ctx: &mut TxContext,
    ): PrivateInventory {
        PrivateInventory {
            id: object::new(ctx),
            commitment: initial_commitment,
            owner: tx_context::sender(ctx),
            nonce: 0,
        }
    }

    /// Verify an item exists in inventory (read-only check)
    public fun verify_item_exists(
        inventory: &PrivateInventory,
        vks: &VerifyingKeys,
        proof: vector<u8>,
        item_id: u32,
        min_quantity: u64,
    ): bool {
        let public_inputs = build_item_exists_inputs(
            &inventory.commitment,
            item_id,
            min_quantity,
        );

        let pvk = groth16::prepare_verifying_key(&vks.curve, &vks.item_exists_vk);
        groth16::verify_groth16_proof(&vks.curve, &pvk, &public_inputs, &proof)
    }

    /// Withdraw items from inventory
    public fun withdraw(
        inventory: &mut PrivateInventory,
        vks: &VerifyingKeys,
        proof: vector<u8>,
        new_commitment: vector<u8>,
        item_id: u32,
        amount: u64,
        ctx: &mut TxContext,
    ) {
        // Only owner can withdraw
        assert!(inventory.owner == tx_context::sender(ctx), ENotOwner);

        let public_inputs = build_withdraw_inputs(
            &inventory.commitment,
            &new_commitment,
            item_id,
            amount,
        );

        let pvk = groth16::prepare_verifying_key(&vks.curve, &vks.withdraw_vk);
        assert!(
            groth16::verify_groth16_proof(&vks.curve, &pvk, &public_inputs, &proof),
            EInvalidProof
        );

        // Update state
        inventory.commitment = new_commitment;
        inventory.nonce = inventory.nonce + 1;

        // Emit event (item_id and amount are public)
        event::emit(WithdrawEvent {
            inventory_id: object::id(inventory),
            item_id,
            amount,
        });
    }

    /// Deposit items into inventory
    public fun deposit(
        inventory: &mut PrivateInventory,
        vks: &VerifyingKeys,
        proof: vector<u8>,
        new_commitment: vector<u8>,
        item_id: u32,
        amount: u64,
    ) {
        // Similar to withdraw, but deposit circuit
    }

    /// Transfer between two inventories
    public fun transfer(
        src: &mut PrivateInventory,
        dst: &mut PrivateInventory,
        vks: &VerifyingKeys,
        proof: vector<u8>,
        src_new_commitment: vector<u8>,
        dst_new_commitment: vector<u8>,
        item_id: u32,
        amount: u64,
        ctx: &mut TxContext,
    ) {
        // Only src owner can initiate
        assert!(src.owner == tx_context::sender(ctx), ENotOwner);

        let public_inputs = build_transfer_inputs(
            &src.commitment,
            &src_new_commitment,
            &dst.commitment,
            &dst_new_commitment,
            item_id,
            amount,
        );

        let pvk = groth16::prepare_verifying_key(&vks.curve, &vks.transfer_vk);
        assert!(
            groth16::verify_groth16_proof(&vks.curve, &pvk, &public_inputs, &proof),
            EInvalidProof
        );

        // Update both inventories
        src.commitment = src_new_commitment;
        src.nonce = src.nonce + 1;
        dst.commitment = dst_new_commitment;
        dst.nonce = dst.nonce + 1;
    }
}
```

### Registry/Factory

```move
// packages/inventory/sources/registry.move

module inventory::registry {
    /// Track all inventories (optional, for discoverability)
    struct InventoryRegistry has key {
        id: UID,
        count: u64,
    }

    /// Spawn a new private inventory
    public fun spawn_inventory(
        registry: &mut InventoryRegistry,
        initial_commitment: vector<u8>,
        ctx: &mut TxContext,
    ): PrivateInventory {
        registry.count = registry.count + 1;

        inventory::create(initial_commitment, ctx)
    }
}
```

---

## API Design

### Proof Server Endpoints

```
POST /api/prove/item-exists
  Body: { inventory, blinding, commitment, item_id, min_quantity }
  Returns: { proof, public_inputs }

POST /api/prove/withdraw
  Body: { old_inventory, new_inventory, old_blinding, new_blinding,
          old_commitment, new_commitment, item_id, amount }
  Returns: { proof, public_inputs }

POST /api/prove/deposit
  Body: { ... }
  Returns: { proof, public_inputs }

POST /api/prove/transfer
  Body: { src_old_inventory, src_new_inventory, ...,
          dst_old_inventory, dst_new_inventory, ...,
          item_id, amount }
  Returns: { proof, public_inputs }

POST /api/commitment/create
  Body: { inventory, blinding }
  Returns: { commitment }

POST /api/blinding/generate
  Returns: { blinding }
```

---

## Test Scenarios

### 1. Basic Flow

```
1. Create inventory with {sword: 10, potion: 5}
2. Prove: has ≥5 swords ✓
3. Withdraw 3 swords
4. Prove: has ≥5 swords ✗ (only 7 now, wait... should be ✓)
5. Prove: has ≥8 swords ✗
```

### 2. Multi-Inventory

```
1. Spawn inventory A: {gold: 100}
2. Spawn inventory B: {empty}
3. Transfer 50 gold: A → B
4. Verify A: has ≥50 gold ✓
5. Verify B: has ≥50 gold ✓
6. Verify A: has ≥60 gold ✗
```

### 3. Edge Cases

```
- Withdraw more than available (proof fails)
- Withdraw from wrong commitment (proof fails)
- Replay old proof (nonce mismatch)
- Transfer to self (should work)
- Empty inventory operations
```

---

## Estimated Complexity

| Component | Constraints | Proving Time |
|-----------|-------------|--------------|
| ItemExistsCircuit | ~400 | ~40ms |
| WithdrawCircuit | ~600 | ~60ms |
| DepositCircuit | ~600 | ~60ms |
| TransferCircuit | ~1000 | ~100ms |

---

## Implementation Order

```
Phase 1: Core (Week 1)
  ├── Inventory data structure
  ├── Poseidon commitment
  ├── ItemExistsCircuit
  └── Basic Move contract

Phase 2: State Transitions (Week 2)
  ├── WithdrawCircuit
  ├── DepositCircuit
  └── Move withdraw/deposit functions

Phase 3: Transfers (Week 3)
  ├── TransferCircuit
  └── Move transfer function

Phase 4: Polish (Week 4)
  ├── Proof server
  ├── E2E tests
  └── Documentation
```

---

## Key Differences from location-privacy

| Aspect | location-privacy | inventory-privacy |
|--------|------------------|-------------------|
| Data structure | 3 coordinates | N item slots |
| Commitment | Poseidon(x,y,z,r) | Poseidon(slots...,r) |
| Main proof | Distance check | Quantity check |
| State changes | No | Yes (withdraw/deposit) |
| Multi-object | No | Yes (transfers) |
| Circuits | 1 | 4 |

---

*Source: Question from ZK Study Session*
