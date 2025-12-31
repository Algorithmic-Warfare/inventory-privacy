import { Transaction } from '@mysten/sui/transactions';

// Re-export Transaction type for consistency
export { Transaction };
import { INVENTORY_MODULE } from './config';

/**
 * Build transaction to create a new private inventory
 */
export function buildCreateInventoryTx(
  packageId: string,
  commitment: Uint8Array,
  recipient?: string
): Transaction {
  const tx = new Transaction();

  // Create the inventory
  const [inventory] = tx.moveCall({
    target: `${packageId}::${INVENTORY_MODULE}::create`,
    arguments: [tx.pure.vector('u8', Array.from(commitment))],
  });

  // Transfer the created inventory to the sender (or specified recipient)
  if (recipient) {
    tx.transferObjects([inventory], recipient);
  }
  // If no recipient, the inventory stays owned by the transaction sender automatically

  return tx;
}

/**
 * Build transaction to verify item exists (read-only, no state change, SMT-based)
 */
export function buildVerifyItemExistsTx(
  packageId: string,
  inventoryId: string,
  verifyingKeysId: string,
  proof: Uint8Array,
  publicHash: Uint8Array
): Transaction {
  const tx = new Transaction();

  tx.moveCall({
    target: `${packageId}::${INVENTORY_MODULE}::verify_item_exists`,
    arguments: [
      tx.object(inventoryId),
      tx.object(verifyingKeysId),
      tx.pure.vector('u8', Array.from(proof)),
      tx.pure.vector('u8', Array.from(publicHash)),
    ],
  });

  return tx;
}

/**
 * Build transaction to withdraw items (SMT-based with signal hash)
 * Now includes nonce, inventoryId, and registryRoot for security
 */
export function buildWithdrawTx(
  packageId: string,
  inventoryId: string,
  registryId: string,
  verifyingKeysId: string,
  proof: Uint8Array,
  signalHash: Uint8Array,
  proofNonce: bigint,
  proofInventoryId: Uint8Array,
  proofRegistryRoot: Uint8Array,
  newCommitment: Uint8Array,
  itemId: number,
  amount: bigint
): Transaction {
  const tx = new Transaction();

  tx.moveCall({
    target: `${packageId}::${INVENTORY_MODULE}::withdraw`,
    arguments: [
      tx.object(inventoryId),
      tx.object(registryId),
      tx.object(verifyingKeysId),
      tx.pure.vector('u8', Array.from(proof)),
      tx.pure.vector('u8', Array.from(signalHash)),
      tx.pure.u64(proofNonce),
      tx.pure.vector('u8', Array.from(proofInventoryId)),
      tx.pure.vector('u8', Array.from(proofRegistryRoot)),
      tx.pure.vector('u8', Array.from(newCommitment)),
      tx.pure.u64(itemId),
      tx.pure.u64(amount),
    ],
  });

  return tx;
}

/**
 * Build transaction to deposit items (SMT-based with signal hash)
 * Now includes nonce, inventoryId, and registryRoot for security
 */
export function buildDepositTx(
  packageId: string,
  inventoryId: string,
  registryId: string,
  verifyingKeysId: string,
  proof: Uint8Array,
  signalHash: Uint8Array,
  proofNonce: bigint,
  proofInventoryId: Uint8Array,
  proofRegistryRoot: Uint8Array,
  newCommitment: Uint8Array,
  itemId: number,
  amount: bigint
): Transaction {
  const tx = new Transaction();

  tx.moveCall({
    target: `${packageId}::${INVENTORY_MODULE}::deposit`,
    arguments: [
      tx.object(inventoryId),
      tx.object(registryId),
      tx.object(verifyingKeysId),
      tx.pure.vector('u8', Array.from(proof)),
      tx.pure.vector('u8', Array.from(signalHash)),
      tx.pure.u64(proofNonce),
      tx.pure.vector('u8', Array.from(proofInventoryId)),
      tx.pure.vector('u8', Array.from(proofRegistryRoot)),
      tx.pure.vector('u8', Array.from(newCommitment)),
      tx.pure.u64(itemId),
      tx.pure.u64(amount),
    ],
  });

  return tx;
}

/**
 * Build transaction to transfer items between inventories (SMT-based with signal hashes)
 * Now includes nonce, inventoryId, and registryRoot for both source and destination
 */
export function buildTransferTx(
  packageId: string,
  srcInventoryId: string,
  dstInventoryId: string,
  registryId: string,
  verifyingKeysId: string,
  // Source proof parameters
  srcProof: Uint8Array,
  srcSignalHash: Uint8Array,
  srcNonce: bigint,
  srcProofInventoryId: Uint8Array,
  srcRegistryRoot: Uint8Array,
  srcNewCommitment: Uint8Array,
  // Destination proof parameters
  dstProof: Uint8Array,
  dstSignalHash: Uint8Array,
  dstNonce: bigint,
  dstProofInventoryId: Uint8Array,
  dstRegistryRoot: Uint8Array,
  dstNewCommitment: Uint8Array,
  // Transfer metadata
  itemId: number,
  amount: bigint
): Transaction {
  const tx = new Transaction();

  tx.moveCall({
    target: `${packageId}::${INVENTORY_MODULE}::transfer`,
    arguments: [
      tx.object(srcInventoryId),
      tx.object(dstInventoryId),
      tx.object(registryId),
      tx.object(verifyingKeysId),
      // Source (withdraw) parameters
      tx.pure.vector('u8', Array.from(srcProof)),
      tx.pure.vector('u8', Array.from(srcSignalHash)),
      tx.pure.u64(srcNonce),
      tx.pure.vector('u8', Array.from(srcProofInventoryId)),
      tx.pure.vector('u8', Array.from(srcRegistryRoot)),
      tx.pure.vector('u8', Array.from(srcNewCommitment)),
      // Destination (deposit) parameters
      tx.pure.vector('u8', Array.from(dstProof)),
      tx.pure.vector('u8', Array.from(dstSignalHash)),
      tx.pure.u64(dstNonce),
      tx.pure.vector('u8', Array.from(dstProofInventoryId)),
      tx.pure.vector('u8', Array.from(dstRegistryRoot)),
      tx.pure.vector('u8', Array.from(dstNewCommitment)),
      // Transfer metadata
      tx.pure.u64(itemId),
      tx.pure.u64(amount),
    ],
  });

  return tx;
}

/**
 * Convert hex string to Uint8Array
 */
export function hexToBytes(hex: string): Uint8Array {
  const cleanHex = hex.startsWith('0x') ? hex.slice(2) : hex;
  const bytes = new Uint8Array(cleanHex.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = parseInt(cleanHex.substr(i * 2, 2), 16);
  }
  return bytes;
}

/**
 * Convert Uint8Array to hex string
 */
export function bytesToHex(bytes: Uint8Array): string {
  return '0x' + Array.from(bytes).map(b => b.toString(16).padStart(2, '0')).join('');
}

// ============ Capacity-Aware Transaction Builders ============

const VOLUME_REGISTRY_MODULE = 'volume_registry';

/**
 * Build transaction to create a new private inventory with capacity
 */
export function buildCreateInventoryWithCapacityTx(
  packageId: string,
  commitment: Uint8Array,
  maxCapacity: bigint,
  recipient?: string
): Transaction {
  const tx = new Transaction();

  // Create the inventory with capacity
  const [inventory] = tx.moveCall({
    target: `${packageId}::${INVENTORY_MODULE}::create_with_capacity`,
    arguments: [
      tx.pure.vector('u8', Array.from(commitment)),
      tx.pure.u64(maxCapacity),
    ],
  });

  // Transfer the created inventory to the sender (or specified recipient)
  if (recipient) {
    tx.transferObjects([inventory], recipient);
  }
  // If no recipient, the inventory stays owned by the transaction sender automatically

  return tx;
}

/**
 * Build transaction to deposit items with capacity check
 */
export function buildDepositWithCapacityTx(
  packageId: string,
  inventoryId: string,
  registryId: string,
  verifyingKeysId: string,
  proof: Uint8Array,
  newCommitment: Uint8Array,
  itemId: number,
  amount: bigint
): Transaction {
  const tx = new Transaction();

  tx.moveCall({
    target: `${packageId}::${INVENTORY_MODULE}::deposit_with_capacity`,
    arguments: [
      tx.object(inventoryId),
      tx.object(registryId),
      tx.object(verifyingKeysId),
      tx.pure.vector('u8', Array.from(proof)),
      tx.pure.vector('u8', Array.from(newCommitment)),
      tx.pure.u32(itemId),
      tx.pure.u64(amount),
    ],
  });

  return tx;
}

/**
 * Build transaction to transfer items with destination capacity check
 */
export function buildTransferWithCapacityTx(
  packageId: string,
  srcInventoryId: string,
  dstInventoryId: string,
  registryId: string,
  verifyingKeysId: string,
  proof: Uint8Array,
  srcNewCommitment: Uint8Array,
  dstNewCommitment: Uint8Array,
  itemId: number,
  amount: bigint
): Transaction {
  const tx = new Transaction();

  tx.moveCall({
    target: `${packageId}::${INVENTORY_MODULE}::transfer_with_capacity`,
    arguments: [
      tx.object(srcInventoryId),
      tx.object(dstInventoryId),
      tx.object(registryId),
      tx.object(verifyingKeysId),
      tx.pure.vector('u8', Array.from(proof)),
      tx.pure.vector('u8', Array.from(srcNewCommitment)),
      tx.pure.vector('u8', Array.from(dstNewCommitment)),
      tx.pure.u32(itemId),
      tx.pure.u64(amount),
    ],
  });

  return tx;
}

/**
 * Build transaction to verify capacity proof
 */
export function buildVerifyCapacityTx(
  packageId: string,
  inventoryId: string,
  registryId: string,
  verifyingKeysId: string,
  proof: Uint8Array
): Transaction {
  const tx = new Transaction();

  tx.moveCall({
    target: `${packageId}::${INVENTORY_MODULE}::verify_capacity`,
    arguments: [
      tx.object(inventoryId),
      tx.object(registryId),
      tx.object(verifyingKeysId),
      tx.pure.vector('u8', Array.from(proof)),
    ],
  });

  return tx;
}

/**
 * Build transaction to create a volume registry
 */
export function buildCreateVolumeRegistryTx(
  packageId: string,
  volumes: bigint[],
  registryHash: Uint8Array,
  recipient?: string
): Transaction {
  const tx = new Transaction();

  const volumesArg = tx.pure.vector('u64', volumes.map(v => Number(v)));

  const [registry] = tx.moveCall({
    target: `${packageId}::${VOLUME_REGISTRY_MODULE}::create`,
    arguments: [
      volumesArg,
      tx.pure.vector('u8', Array.from(registryHash)),
    ],
  });

  // Transfer the created registry to the sender (or specified recipient)
  if (recipient) {
    tx.transferObjects([registry], recipient);
  }
  // If no recipient, the registry stays owned by the transaction sender automatically

  return tx;
}
