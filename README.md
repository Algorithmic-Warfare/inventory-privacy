# Inventory Privacy

Private on-chain inventory state with verifiable ZK operations using Sparse Merkle Trees and Groth16 proofs.

## Overview

This project implements private inventories on Sui where:
- Inventory contents are hidden (only an SMT root commitment is stored on-chain)
- Operations are verifiable via Groth16 ZK proofs
- State transitions are proven correct without revealing actual contents

```
┌─────────────────────────────────────────────────────────────────┐
│                      INVENTORY PRIVACY                          │
│                                                                 │
│  On-chain:   SMT Root (32 bytes)                               │
│  Off-chain:  Full inventory state + Merkle proofs              │
│                                                                 │
│  Proves:                                                        │
│    - "I have item X at quantity Q" (membership)                │
│    - "I can deposit/withdraw X" (valid state transition)       │
│    - "My inventory is within capacity" (volume check)          │
│                                                                 │
│  Reveals: Nothing except the statement is true                 │
└─────────────────────────────────────────────────────────────────┘
```

## Architecture

```
inventory-privacy/
├── crates/
│   ├── circuits/          # ZK circuits (arkworks, uses anemoi)
│   ├── prover/            # Proof generation library
│   └── proof-server/      # HTTP API for proof generation
├── packages/
│   └── inventory/         # Sui Move contracts
├── scripts/               # Setup and deployment scripts
└── keys/                  # Generated proving/verifying keys
```

### External Dependencies

| Crate | Description | Repository |
|-------|-------------|------------|
| `anemoi` | ZK-friendly hash function | [github.com/abderraouf-belalia/anemoi](https://github.com/abderraouf-belalia/anemoi) |

## Circuits

All circuits use the **Anemoi** hash function (CRYPTO 2023) for ~2x constraint reduction compared to Poseidon.

| Circuit | Purpose | Constraints | Public Inputs |
|---------|---------|-------------|---------------|
| `StateTransitionCircuit` | Prove valid deposit/withdraw with capacity check | ~7,520 | signal_hash (compressed) |
| `ItemExistsSMTCircuit` | Prove inventory contains item at quantity | ~2,180 | root, item_id, quantity, signal_hash |
| `CapacitySMTCircuit` | Prove inventory volume is within capacity | ~379 | root, capacity, signal_hash |

### Commitment Scheme

Inventories are committed using a **Sparse Merkle Tree** (depth 16):

```
                    root
                   /    \
                ...      ...
               /            \
         leaf[slot_i]    leaf[slot_j]
              |               |
    hash(item_id, qty)  hash(item_id, qty)
```

- Each slot is a leaf: `hash(item_id, quantity)`
- Empty slots use a canonical empty hash
- Only the root is stored on-chain (~32 bytes)

## Getting Started

### Prerequisites

- Rust 1.75+
- Sui CLI
- Node.js (for deployment scripts)

### Build

```bash
# Build all Rust crates
cargo build --release

# Build Move contracts
cd packages/inventory && sui move build
```

### Run Tests

```bash
# Run all Rust tests
cargo test --all

# Run Move tests
cd packages/inventory && sui move test
```

### Generate Keys

```bash
# Run trusted setup (generates proving/verifying keys)
cargo run --release -p inventory-prover --example setup
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

### Generate State Transition Proof
```bash
curl -X POST http://localhost:3000/api/prove/state-transition \
  -H "Content-Type: application/json" \
  -d '{
    "old_root": "0x...",
    "new_root": "0x...",
    "item_id": 1,
    "old_quantity": 100,
    "new_quantity": 70,
    "merkle_proof": [...],
    "capacity": 1000,
    "current_volume": 500
  }'
```

### Generate Item Exists Proof
```bash
curl -X POST http://localhost:3000/api/prove/item-exists \
  -H "Content-Type: application/json" \
  -d '{
    "root": "0x...",
    "item_id": 1,
    "quantity": 100,
    "merkle_proof": [...]
  }'
```

## Data Flow

```
1. User has off-chain inventory state (full SMT)
2. User wants to withdraw item X, quantity Q
3. User generates Merkle proof for slot containing X
4. Prover generates Groth16 proof:
   - Proves old_root contains (X, old_qty) at slot
   - Proves new_qty = old_qty - Q (no underflow)
   - Proves new_root is correct after update
   - Proves volume stays within capacity
5. On-chain verifier checks proof
6. Contract updates stored root: old_root → new_root
```

## Security Model

**What's hidden:**
- Which items are in the inventory
- Quantities of each item
- Inventory structure and slot assignments

**What's revealed:**
- Frequency of operations (state transitions)
- That a valid operation occurred

**Trusted Setup:** The current implementation uses ceremony-generated parameters. Production deployments should use a proper multi-party trusted setup.

## Performance

| Operation | Proving Time | Proof Size |
|-----------|--------------|------------|
| State Transition | ~800ms | 192 bytes |
| Item Exists | ~300ms | 192 bytes |
| Capacity Check | ~100ms | 192 bytes |

*Measured on Apple M1. Parallel proof generation supported.*

## Related Projects

| Project | Description |
|---------|-------------|
| [anemoi](https://github.com/abderraouf-belalia/anemoi) | ZK-friendly hash function |
| [r1cs-optimizer](https://github.com/abderraouf-belalia/r1cs-optimizer) | R1CS constraint optimizer |

## License

MIT
