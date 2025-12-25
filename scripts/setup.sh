#!/bin/bash
# Generate proving and verifying keys for all circuits

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
KEYS_DIR="$PROJECT_ROOT/keys"

echo "=== Inventory Privacy - Trusted Setup ==="
echo ""

# Check if keys already exist
if [ -d "$KEYS_DIR" ] && [ -f "$KEYS_DIR/item_exists.pk" ]; then
    echo "Keys already exist in $KEYS_DIR"
    read -p "Regenerate? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Keeping existing keys."
        exit 0
    fi
    rm -rf "$KEYS_DIR"
fi

# Create keys directory
mkdir -p "$KEYS_DIR"

echo "Building prover..."
cd "$PROJECT_ROOT"
cargo build --release -p inventory-prover

echo ""
echo "Running trusted setup (this may take a few minutes)..."
echo ""

# Run setup binary (we'll create a simple setup binary)
cargo run --release --bin setup-keys -- "$KEYS_DIR"

echo ""
echo "=== Setup Complete ==="
echo "Keys saved to: $KEYS_DIR"
echo ""
ls -la "$KEYS_DIR"
