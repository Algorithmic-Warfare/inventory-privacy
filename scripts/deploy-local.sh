#!/bin/bash
# Deploy inventory-privacy contracts to local Sui network
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

echo "=== Inventory Privacy Local Deployment ==="
echo ""

# Check if sui is installed
if ! command -v sui &> /dev/null; then
    echo "Error: sui CLI not found. Please install it first:"
    echo "  cargo install --locked --git https://github.com/MystenLabs/sui.git --branch devnet sui"
    exit 1
fi

# Check if local network is running
if ! sui client envs | grep -q "localnet"; then
    echo "Setting up localnet environment..."
    sui client new-env --alias localnet --rpc http://127.0.0.1:9000
fi

echo "Switching to localnet..."
sui client switch --env localnet

# Check balance, request from faucet if needed
BALANCE=$(sui client gas --json 2>/dev/null | jq -r '.[0].mistBalance // "0"')
if [ "$BALANCE" == "0" ] || [ "$BALANCE" == "null" ]; then
    echo "Requesting gas from local faucet..."
    sui client faucet --url http://127.0.0.1:9123/gas || echo "Faucet request failed, you may need to fund manually"
fi

# Export verifying keys
echo ""
echo "Exporting verifying keys..."
cd "$ROOT_DIR"
cargo run --release --bin export-vks

# Check if keys were exported
if [ ! -f "keys/verifying_keys.json" ]; then
    echo "Error: Verifying keys not exported"
    exit 1
fi

# Build Move package
echo ""
echo "Building Move package..."
cd "$ROOT_DIR/packages/inventory"
sui move build

# Publish package
echo ""
echo "Publishing package..."
PUBLISH_OUTPUT=$(sui client publish --gas-budget 500000000 --json)

PACKAGE_ID=$(echo "$PUBLISH_OUTPUT" | jq -r '.objectChanges[] | select(.type == "published") | .packageId')

if [ -z "$PACKAGE_ID" ] || [ "$PACKAGE_ID" == "null" ]; then
    echo "Error: Failed to extract package ID"
    echo "$PUBLISH_OUTPUT"
    exit 1
fi

echo "Package published: $PACKAGE_ID"

# Save deployment info
DEPLOY_INFO="$ROOT_DIR/keys/deployment.json"
cat > "$DEPLOY_INFO" << EOF
{
  "network": "localnet",
  "packageId": "$PACKAGE_ID",
  "timestamp": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
}
EOF

echo ""
echo "=== Deployment Complete ==="
echo "Package ID: $PACKAGE_ID"
echo "Deployment info saved to: $DEPLOY_INFO"
echo ""
echo "Next steps:"
echo "1. Initialize verifying keys by calling init_verifying_keys"
echo "2. Configure the web UI with the package ID"
echo "3. Run: npm run dev in the web directory"
