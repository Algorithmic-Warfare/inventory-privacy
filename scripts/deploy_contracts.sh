#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

echo "=== Inventory Privacy Contract Deployment ==="

# Ensure we're on localnet
sui client switch --env localnet 2>/dev/null || true

# Check for verifying keys
VKS_PATH="$ROOT_DIR/keys/verifying_keys.json"
if [[ ! -f "$VKS_PATH" ]]; then
  echo "[deploy] Verifying keys not found. Running export-vks..."
  cd "$ROOT_DIR"
  cargo run --release --bin export-vks
fi

if [[ ! -f "$VKS_PATH" ]]; then
  echo "[deploy] ERROR: Verifying keys still not found after export" >&2
  exit 1
fi

echo "[deploy] Verifying keys ready"

# Publish Move package
echo "[deploy] Publishing Move package..."
cd "$ROOT_DIR/packages/inventory"

PUBLISH_OUTPUT=$(sui client publish --gas-budget 500000000 --json 2>&1)
PACKAGE_ID=$(echo "$PUBLISH_OUTPUT" | grep -oP '"packageId"\s*:\s*"\K[^"]+' | head -1)

if [[ -z "$PACKAGE_ID" ]]; then
  echo "[deploy] ERROR: Failed to extract package ID" >&2
  echo "$PUBLISH_OUTPUT"
  exit 1
fi

echo "[deploy] Package published: $PACKAGE_ID"

# Extract VK hex values
cd "$ROOT_DIR"
ITEM_EXISTS_VK=$(node -e "console.log(require('./keys/verifying_keys.json').item_exists_vk)")
WITHDRAW_VK=$(node -e "console.log(require('./keys/verifying_keys.json').withdraw_vk)")
DEPOSIT_VK=$(node -e "console.log(require('./keys/verifying_keys.json').deposit_vk)")
TRANSFER_VK=$(node -e "console.log(require('./keys/verifying_keys.json').transfer_vk)")
CAPACITY_VK=$(node -e "console.log(require('./keys/verifying_keys.json').capacity_vk)")
DEPOSIT_CAPACITY_VK=$(node -e "console.log(require('./keys/verifying_keys.json').deposit_capacity_vk)")
TRANSFER_CAPACITY_VK=$(node -e "console.log(require('./keys/verifying_keys.json').transfer_capacity_vk)")

# Create VolumeRegistry
echo "[deploy] Creating VolumeRegistry..."
VOL_RESULT=$(sui client call \
  --package "$PACKAGE_ID" \
  --module volume_registry \
  --function create_and_share \
  --args '[0,5,3,8,2,10,4,15,1,6,7,12,9,20,11,25]' '0xb08a402d53183775208f9f8772791a51f6af5f7b648203b9bef158feb89b1815' \
  --gas-budget 100000000 \
  --json 2>&1)

VOLUME_REGISTRY_ID=$(echo "$VOL_RESULT" | grep -oP '"objectId"\s*:\s*"\K[^"]+' | head -1)
echo "[deploy] VolumeRegistry: $VOLUME_REGISTRY_ID"

# Create VerifyingKeys
echo "[deploy] Creating VerifyingKeys..."
VK_RESULT=$(sui client call \
  --package "$PACKAGE_ID" \
  --module inventory \
  --function init_verifying_keys_and_share \
  --args "$ITEM_EXISTS_VK" "$WITHDRAW_VK" "$DEPOSIT_VK" "$TRANSFER_VK" "$CAPACITY_VK" "$DEPOSIT_CAPACITY_VK" "$TRANSFER_CAPACITY_VK" \
  --gas-budget 500000000 \
  --json 2>&1)

VERIFYING_KEYS_ID=$(echo "$VK_RESULT" | grep -oP '"objectId"\s*:\s*"\K[^"]+' | head -1)
echo "[deploy] VerifyingKeys: $VERIFYING_KEYS_ID"

# Save deployment info
DEPLOY_FILE="$ROOT_DIR/keys/deployment.json"
cat > "$DEPLOY_FILE" << EOF
{
  "network": "localnet",
  "packageId": "$PACKAGE_ID",
  "verifyingKeysId": "$VERIFYING_KEYS_ID",
  "volumeRegistryId": "$VOLUME_REGISTRY_ID",
  "timestamp": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
}
EOF

echo ""
echo "=== Deployment Complete ==="
echo "Package ID: $PACKAGE_ID"
echo "VerifyingKeys ID: $VERIFYING_KEYS_ID"
echo "VolumeRegistry ID: $VOLUME_REGISTRY_ID"
echo ""
echo "Update web/src/sui/config.ts with:"
echo "  packageId: '$PACKAGE_ID',"
echo "  verifyingKeysId: '$VERIFYING_KEYS_ID',"
echo "  volumeRegistryId: '$VOLUME_REGISTRY_ID',"
echo ""
echo "Or configure via web UI: http://localhost:5173/on-chain"
