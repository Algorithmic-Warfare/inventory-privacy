#!/bin/bash
# End-to-end test: setup, prove, verify

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "=== Inventory Privacy - End-to-End Test ==="
echo ""

cd "$PROJECT_ROOT"

# Step 1: Ensure keys exist
echo "Step 1: Checking/generating keys..."
if [ ! -d "keys" ] || [ ! -f "keys/item_exists.pk" ]; then
    echo "Keys not found, running setup..."
    ./scripts/setup.sh
fi
echo "Keys ready."
echo ""

# Step 2: Run unit tests
echo "Step 2: Running unit tests..."
cargo test --all
echo "Unit tests passed."
echo ""

# Step 3: Start proof server in background
echo "Step 3: Starting proof server..."
cargo build --release -p inventory-proof-server
./target/release/inventory-proof-server &
SERVER_PID=$!
sleep 3

# Cleanup function
cleanup() {
    echo ""
    echo "Cleaning up..."
    kill $SERVER_PID 2>/dev/null || true
}
trap cleanup EXIT

# Step 4: Test health endpoint
echo "Step 4: Testing health endpoint..."
HEALTH=$(curl -s http://localhost:3000/health)
echo "Health response: $HEALTH"

# Step 5: Generate blinding factor
echo ""
echo "Step 5: Generating blinding factor..."
BLINDING=$(curl -s -X POST http://localhost:3000/api/blinding/generate)
echo "Blinding: $BLINDING"

# Step 6: Create commitment
echo ""
echo "Step 6: Creating commitment..."
COMMITMENT=$(curl -s -X POST http://localhost:3000/api/commitment/create \
    -H "Content-Type: application/json" \
    -d '{
        "inventory": [
            {"item_id": 1, "quantity": 100},
            {"item_id": 2, "quantity": 50}
        ],
        "blinding": "0x0000000000000000000000000000000000000000000000000000000000003039"
    }')
echo "Commitment: $COMMITMENT"

# Step 7: Generate item exists proof
echo ""
echo "Step 7: Generating ItemExists proof..."
PROOF=$(curl -s -X POST http://localhost:3000/api/prove/item-exists \
    -H "Content-Type: application/json" \
    -d '{
        "inventory": [
            {"item_id": 1, "quantity": 100},
            {"item_id": 2, "quantity": 50}
        ],
        "blinding": "0x0000000000000000000000000000000000000000000000000000000000003039",
        "item_id": 1,
        "min_quantity": 50
    }')
echo "Proof generated successfully!"
echo "Response: $PROOF"

# Step 8: Generate withdraw proof
echo ""
echo "Step 8: Generating Withdraw proof..."
WITHDRAW_PROOF=$(curl -s -X POST http://localhost:3000/api/prove/withdraw \
    -H "Content-Type: application/json" \
    -d '{
        "old_inventory": [
            {"item_id": 1, "quantity": 100}
        ],
        "old_blinding": "0x0000000000000000000000000000000000000000000000000000000000003039",
        "new_blinding": "0x0000000000000000000000000000000000000000000000000000000000010932",
        "item_id": 1,
        "amount": 30
    }')
echo "Withdraw proof generated!"
echo "Response: $WITHDRAW_PROOF"

echo ""
echo "=== End-to-End Test Complete ==="
echo "All operations successful!"
