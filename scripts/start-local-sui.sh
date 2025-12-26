#!/bin/bash
# Start a local Sui network for testing
set -e

echo "=== Starting Local Sui Network ==="
echo ""

# Check if sui is installed
if ! command -v sui &> /dev/null; then
    echo "Error: sui CLI not found. Please install it first:"
    echo ""
    echo "  # Using cargo (recommended):"
    echo "  cargo install --locked --git https://github.com/MystenLabs/sui.git --branch devnet sui"
    echo ""
    echo "  # Or download pre-built binaries from:"
    echo "  https://github.com/MystenLabs/sui/releases"
    echo ""
    exit 1
fi

echo "Sui version: $(sui --version)"
echo ""

# Start local network
echo "Starting local Sui network..."
echo "This will run in the foreground. Press Ctrl+C to stop."
echo ""
echo "Network will be available at:"
echo "  RPC: http://127.0.0.1:9000"
echo "  Faucet: http://127.0.0.1:9123"
echo ""

sui start --with-faucet
