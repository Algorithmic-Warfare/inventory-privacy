# Inventory Privacy

Private on-chain inventory state with verifiable ZK operations using Sparse Merkle Trees and Groth16 proofs.

## Overview

This project implements private inventories on Sui where:
- Inventory contents are hidden (only an SMT commitment is stored on-chain)
- Operations are verifiable via Groth16 ZK proofs
- State transitions are proven correct without revealing actual contents

```
┌─────────────────────────────────────────────────────────────────┐
│                      INVENTORY PRIVACY                          │
│                                                                 │
│  On-chain:   Commitment = Poseidon(root, volume, blinding)     │
│  Off-chain:  Full inventory state + Merkle proofs              │
│                                                                 │
│  Proves:                                                        │
│    - "I have >= N of item X" (membership)                      │
│    - "I can deposit/withdraw X" (valid state transition)       │
│    - "My inventory is within capacity" (volume check)          │
│                                                                 │
│  Reveals: Nothing except the statement is true                 │
└─────────────────────────────────────────────────────────────────┘
```

## Screenshots

### Home Page
![Home Page](docs/screenshots/home.png)

### Create Inventory
![Create Inventory](docs/screenshots/create-inventory.png)

### Deposit/Withdraw Operations
![Operations](docs/screenshots/operations.png)

### Private Transfer
![Transfer](docs/screenshots/transfer.png)

## Architecture

```
inventory-privacy/
├── crates/
│   ├── circuits/          # ZK circuits (arkworks + Poseidon)
│   ├── prover/            # Proof generation library
│   └── proof-server/      # HTTP API for proof generation
├── packages/
│   └── inventory/         # Sui Move contracts
├── web/                   # React frontend
├── scripts/               # Setup and deployment scripts
└── keys/                  # Generated proving/verifying keys
```

## Circuits

All circuits use the **Poseidon** hash function optimized for ZK circuits (~240 constraints per hash).

| Circuit | Purpose | Constraints | Proof Time |
|---------|---------|-------------|------------|
| `StateTransition` | Prove valid deposit/withdraw with capacity check | ~8,597 | ~450ms |
| `ItemExists` | Prove inventory contains >= N of item | ~4,124 | ~200ms |
| `Capacity` | Prove inventory volume is within capacity | ~724 | ~40ms |

### Commitment Scheme

Inventories use a **Sparse Merkle Tree** (depth 12, supports 4,096 item types):

```
Commitment = Poseidon(inventory_root, current_volume, blinding)

                    inventory_root
                       /    \
                    ...      ...
                   /            \
             leaf[slot_i]    leaf[slot_j]
                  |               |
        Poseidon(id, qty)  Poseidon(id, qty)
```

- Each leaf: `Poseidon(item_id, quantity)`
- Empty slots use precomputed `Poseidon(0, 0)`
- Only the commitment is stored on-chain (~32 bytes)
- Volume tracked incrementally for O(1) capacity checks

### Signal Hash Pattern

Sui limits ZK proofs to 8 public inputs. We compress all parameters into one hash:

```
signal_hash = Poseidon(
    old_commitment,
    new_commitment,
    registry_root,
    max_capacity,
    item_id,
    amount,
    op_type,
    nonce,
    inventory_id
)
```

## Getting Started

### Prerequisites

- Rust 1.75+
- Sui CLI
- Node.js 18+ (for web frontend)

### Quick Start

```bash
# Install dependencies
npm install

# Start local Sui network + deploy contracts + start proof server + web
npm run dev
```

This uses `mprocs` to run all services in parallel.

### Manual Setup

```bash
# Build all Rust crates
cargo build --release

# Build Move contracts
cd packages/inventory && sui move build

# Generate proving/verifying keys
cargo run --release -p inventory-prover --bin export-vks

# Deploy to localnet
npm run deploy

# Start proof server
cargo run --release -p inventory-proof-server

# Start web frontend
cd web && npm run dev
```

### Run Tests

```bash
# Run all Rust tests (85 tests)
cargo test --release

# Run Move tests
cd packages/inventory && sui move test
```

## API Endpoints

### Health Check
```bash
curl http://localhost:3001/health
```

### Generate State Transition Proof
```bash
curl -X POST http://localhost:3001/prove/state-transition \
  -H "Content-Type: application/json" \
  -d '{
    "old_root": "0x...",
    "new_root": "0x...",
    "item_id": 1,
    "old_quantity": 100,
    "new_quantity": 70,
    "amount": 30,
    "op_type": "withdraw",
    ...
  }'
```

### Create Inventory Commitment
```bash
curl -X POST http://localhost:3001/inventory/create \
  -H "Content-Type: application/json" \
  -d '{
    "max_capacity": 1000,
    "initial_items": [{"item_id": 1, "quantity": 100}]
  }'
```

## Security Model

**What's hidden:**
- Which items are in the inventory
- Quantities of each item
- Total volume used
- Inventory structure and slot assignments

**What's revealed:**
- Frequency of operations (state transitions)
- That a valid operation occurred
- Max capacity (public parameter)

**Attack Prevention:**
| Attack | Prevention |
|--------|------------|
| Replay | Nonce in signal hash, verified on-chain |
| Cross-inventory | Inventory ID in signal hash |
| Underflow | 32-bit range checks on quantities |
| Capacity bypass | Explicit capacity check in circuit |

## Performance

| Operation | Proofs | Proving Time | Verification | Gas |
|-----------|--------|--------------|--------------|-----|
| Deposit | 1 | ~450ms | ~250ms | 0.001 SUI |
| Withdraw | 1 | ~450ms | ~250ms | 0.001 SUI |
| Transfer | 2 | ~800ms | ~300ms | 0.001 SUI |
| Prove Ownership | 1 | ~200ms | ~200ms | - |

*Measured on modern hardware. Proofs generated in parallel where applicable.*

## Documentation

- [Circuit Architecture](docs/circuits/README.md) - Deep dive into circuit design
- [StateTransition Circuit](docs/circuits/state_transition.md) - Line-by-line breakdown
- [ItemExists Circuit](docs/circuits/item_exists.md) - Membership proof details
- [Capacity Circuit](docs/circuits/capacity.md) - Volume verification
- [Supporting Gadgets](docs/circuits/gadgets.md) - Poseidon, SMT, range checks

## Tech Stack

- **Circuits**: arkworks (ark-groth16, ark-bn254, ark-r1cs-std)
- **Hash Function**: Poseidon (ZK-optimized, ~240 constraints)
- **Blockchain**: Sui (Move contracts with native Groth16 verifier)
- **Frontend**: React + TypeScript + Vite
- **Proof Server**: Rust + Axum

## Platform Support

This project has been primarily tested on **Windows**. It may require adjustments for other platforms (Linux, macOS). PRs for cross-platform support are welcome!

## License

MIT
