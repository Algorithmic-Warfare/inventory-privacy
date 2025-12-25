# Inventory Privacy PoC

A proof-of-concept demonstrating hidden on-chain inventory state with verifiable ZK operations.

## Overview

This project implements private inventories on Sui where:
- Inventory contents are hidden (only a Poseidon commitment is stored on-chain)
- Operations are verifiable via Groth16 ZK proofs
- State transitions are proven correct without revealing actual quantities

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

## Architecture

```
inventory-privacy/
├── crates/
│   ├── circuits/          # ZK circuit definitions (arkworks)
│   ├── prover/            # Proof generation library
│   └── proof-server/      # HTTP API for proof generation
├── packages/
│   └── inventory/         # Sui Move contracts
├── scripts/               # Setup and test scripts
└── keys/                  # Generated proving/verifying keys
```

## Circuits

| Circuit | Purpose | Public Inputs |
|---------|---------|---------------|
| `ItemExistsCircuit` | Prove inventory contains ≥N of item X | commitment, item_id, min_quantity |
| `WithdrawCircuit` | Prove valid withdrawal with state transition | old/new commitment, item_id, amount |
| `DepositCircuit` | Prove valid deposit with state transition | old/new commitment, item_id, amount |
| `TransferCircuit` | Prove valid transfer between inventories | src/dst old/new commitments, item_id, amount |

## Getting Started

### Prerequisites

- Rust 1.75+
- Sui CLI
- curl (for API testing)

### Build

```bash
# Build all Rust crates
cargo build --release

# Build Move contracts
cd packages/inventory && sui move build
```

### Run Tests

```bash
# Run all tests
./scripts/test.sh

# Or run separately:
cargo test --all
cd packages/inventory && sui move test
```

### Generate Keys

```bash
# Run trusted setup (generates proving/verifying keys)
./scripts/setup.sh
```

### Start Proof Server

```bash
cargo run --release -p inventory-proof-server
# Server runs on http://localhost:3000
```

## API Endpoints

### Health Check
```bash
curl http://localhost:3000/health
```

### Generate Blinding Factor
```bash
curl -X POST http://localhost:3000/api/blinding/generate
```

### Create Commitment
```bash
curl -X POST http://localhost:3000/api/commitment/create \
  -H "Content-Type: application/json" \
  -d '{
    "inventory": [{"item_id": 1, "quantity": 100}],
    "blinding": "0x..."
  }'
```

### Prove Item Exists
```bash
curl -X POST http://localhost:3000/api/prove/item-exists \
  -H "Content-Type: application/json" \
  -d '{
    "inventory": [{"item_id": 1, "quantity": 100}],
    "blinding": "0x...",
    "item_id": 1,
    "min_quantity": 50
  }'
```

### Prove Withdraw
```bash
curl -X POST http://localhost:3000/api/prove/withdraw \
  -H "Content-Type: application/json" \
  -d '{
    "old_inventory": [{"item_id": 1, "quantity": 100}],
    "old_blinding": "0x...",
    "new_blinding": "0x...",
    "item_id": 1,
    "amount": 30
  }'
```

## Data Structures

### Inventory (Rust)
```rust
pub const MAX_ITEM_SLOTS: usize = 16;

pub struct Inventory {
    pub slots: [(item_id: u32, quantity: u64); MAX_ITEM_SLOTS],
}
```

### PrivateInventory (Move)
```move
struct PrivateInventory has key, store {
    id: UID,
    commitment: vector<u8>,  // 32 bytes - Poseidon hash
    owner: address,
    nonce: u64,
}
```

## Commitment Scheme

```
commitment = Poseidon(slot0_id, slot0_qty, slot1_id, slot1_qty, ..., blinding)
```

The commitment hides:
- Which items are in the inventory
- How much of each item exists
- The structure of the inventory

The blinding factor ensures the commitment is hiding even if the inventory contents could be guessed.

## Security Considerations

- **Trusted Setup**: The current implementation uses a simple deterministic setup for PoC. Production use requires a proper trusted setup ceremony.
- **Blinding Factors**: Each state transition should use a fresh blinding factor to prevent linking.
- **Replay Protection**: The nonce prevents replaying old proofs.

## Performance Estimates

| Circuit | Constraints | Proving Time |
|---------|-------------|--------------|
| ItemExistsCircuit | ~400 | ~40ms |
| WithdrawCircuit | ~600 | ~60ms |
| DepositCircuit | ~600 | ~60ms |
| TransferCircuit | ~1000 | ~100ms |

*Estimates based on 16 fixed slots. Actual performance depends on hardware.*

## Future Improvements

1. **Volume-Based Inventory**: Use Sparse Merkle Trees for unbounded item types with volume limits
2. **Batch Operations**: Combine multiple withdrawals/deposits into single proofs
3. **Recursive Proofs**: Aggregate multiple proofs for cheaper on-chain verification
4. **Hardware Acceleration**: Use GPU/FPGA for faster proof generation

## Related

- [location-privacy](../): The original location privacy PoC this is based on
- [ZK Study Notes](../docs/zk-study/): Design documents and Q&A

## License

MIT
