#!/bin/bash
# Run all tests

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "=== Inventory Privacy - Test Suite ==="
echo ""

cd "$PROJECT_ROOT"

echo "--- Running Rust tests ---"
echo ""
cargo test --all

echo ""
echo "--- Running Move tests ---"
echo ""
cd "$PROJECT_ROOT/packages/inventory"
sui move test

echo ""
echo "=== All Tests Passed ==="
