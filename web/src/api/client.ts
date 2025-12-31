import type {
  InventoryItem,
  ProofResult,
  StateTransitionResult,
  ApiError,
} from '../types';

const API_BASE = '/api';

async function handleResponse<T>(response: Response): Promise<T> {
  const data = await response.json();
  if (!response.ok) {
    throw new Error((data as ApiError).error || 'Request failed');
  }
  return data as T;
}

export async function generateBlinding(): Promise<string> {
  const response = await fetch(`${API_BASE}/blinding/generate`, {
    method: 'POST',
  });
  const data = await handleResponse<{ blinding: string }>(response);
  return data.blinding;
}

export interface CreateCommitmentResult {
  commitment: string;
  inventory_root: string;
}

export async function createCommitment(
  inventory: InventoryItem[],
  currentVolume: number,
  blinding: string
): Promise<CreateCommitmentResult> {
  const response = await fetch(`${API_BASE}/commitment/create`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      inventory,
      current_volume: currentVolume,
      blinding,
    }),
  });
  return handleResponse<CreateCommitmentResult>(response);
}

// ============ State Transition (Deposit/Withdraw) ============

export interface StateTransitionRequest {
  inventory: InventoryItem[];
  current_volume: number;
  old_blinding: string;
  new_blinding: string;
  item_id: number;
  amount: number;
  item_volume: number;
  registry_root: string;
  max_capacity: number;
  /** Current nonce from on-chain inventory (for replay protection) */
  nonce: number;
  /** Inventory object ID as hex string (for cross-inventory protection) */
  inventory_id: string;
  op_type: 'deposit' | 'withdraw';
}

export async function proveStateTransition(
  request: StateTransitionRequest
): Promise<StateTransitionResult> {
  const response = await fetch(`${API_BASE}/prove/state-transition`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(request),
  });
  return handleResponse<StateTransitionResult>(response);
}

// Convenience wrapper for deposit
export async function proveDeposit(
  inventory: InventoryItem[],
  currentVolume: number,
  oldBlinding: string,
  newBlinding: string,
  itemId: number,
  amount: number,
  itemVolume: number,
  registryRoot: string,
  maxCapacity: number,
  nonce: number,
  inventoryId: string
): Promise<StateTransitionResult> {
  return proveStateTransition({
    inventory,
    current_volume: currentVolume,
    old_blinding: oldBlinding,
    new_blinding: newBlinding,
    item_id: itemId,
    amount,
    item_volume: itemVolume,
    registry_root: registryRoot,
    max_capacity: maxCapacity,
    nonce,
    inventory_id: inventoryId,
    op_type: 'deposit',
  });
}

// Convenience wrapper for withdraw
export async function proveWithdraw(
  inventory: InventoryItem[],
  currentVolume: number,
  oldBlinding: string,
  newBlinding: string,
  itemId: number,
  amount: number,
  itemVolume: number,
  registryRoot: string,
  maxCapacity: number,
  nonce: number,
  inventoryId: string
): Promise<StateTransitionResult> {
  return proveStateTransition({
    inventory,
    current_volume: currentVolume,
    old_blinding: oldBlinding,
    new_blinding: newBlinding,
    item_id: itemId,
    amount,
    item_volume: itemVolume,
    registry_root: registryRoot,
    max_capacity: maxCapacity,
    nonce,
    inventory_id: inventoryId,
    op_type: 'withdraw',
  });
}

// ============ Item Exists ============

export async function proveItemExists(
  inventory: InventoryItem[],
  currentVolume: number,
  blinding: string,
  itemId: number,
  minQuantity: number
): Promise<ProofResult> {
  const response = await fetch(`${API_BASE}/prove/item-exists`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      inventory,
      current_volume: currentVolume,
      blinding,
      item_id: itemId,
      min_quantity: minQuantity,
    }),
  });
  return handleResponse<ProofResult>(response);
}

// ============ Capacity ============

export async function proveCapacity(
  inventory: InventoryItem[],
  currentVolume: number,
  blinding: string,
  maxCapacity: number
): Promise<ProofResult> {
  const response = await fetch(`${API_BASE}/prove/capacity`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      inventory,
      current_volume: currentVolume,
      blinding,
      max_capacity: maxCapacity,
    }),
  });
  return handleResponse<ProofResult>(response);
}

// ============ Transfer (Two State Transitions) ============

export interface TransferProofs {
  srcProof: ProofResult;
  srcNewCommitment: string;
  srcNewVolume: number;
  /** Source nonce used in proof */
  srcNonce: number;
  /** Source inventory_id used in proof */
  srcInventoryId: string;
  /** Source registry_root used in proof */
  srcRegistryRoot: string;
  dstProof: ProofResult;
  dstNewCommitment: string;
  dstNewVolume: number;
  /** Destination nonce used in proof */
  dstNonce: number;
  /** Destination inventory_id used in proof */
  dstInventoryId: string;
  /** Destination registry_root used in proof */
  dstRegistryRoot: string;
}

export async function proveTransfer(
  srcInventory: InventoryItem[],
  srcCurrentVolume: number,
  srcOldBlinding: string,
  srcNewBlinding: string,
  srcNonce: number,
  srcInventoryId: string,
  dstInventory: InventoryItem[],
  dstCurrentVolume: number,
  dstOldBlinding: string,
  dstNewBlinding: string,
  dstNonce: number,
  dstInventoryId: string,
  itemId: number,
  amount: number,
  itemVolume: number,
  registryRoot: string,
  srcMaxCapacity: number,
  dstMaxCapacity: number
): Promise<TransferProofs> {
  // Generate withdrawal proof from source
  const srcResult = await proveWithdraw(
    srcInventory,
    srcCurrentVolume,
    srcOldBlinding,
    srcNewBlinding,
    itemId,
    amount,
    itemVolume,
    registryRoot,
    srcMaxCapacity,
    srcNonce,
    srcInventoryId
  );

  // Generate deposit proof to destination
  const dstResult = await proveDeposit(
    dstInventory,
    dstCurrentVolume,
    dstOldBlinding,
    dstNewBlinding,
    itemId,
    amount,
    itemVolume,
    registryRoot,
    dstMaxCapacity,
    dstNonce,
    dstInventoryId
  );

  return {
    srcProof: {
      proof: srcResult.proof,
      public_inputs: srcResult.public_inputs,
    },
    srcNewCommitment: srcResult.new_commitment,
    srcNewVolume: srcResult.new_volume,
    srcNonce: srcResult.nonce,
    srcInventoryId: srcResult.inventory_id,
    srcRegistryRoot: srcResult.registry_root,
    dstProof: {
      proof: dstResult.proof,
      public_inputs: dstResult.public_inputs,
    },
    dstNewCommitment: dstResult.new_commitment,
    dstNewVolume: dstResult.new_volume,
    dstNonce: dstResult.nonce,
    dstInventoryId: dstResult.inventory_id,
    dstRegistryRoot: dstResult.registry_root,
  };
}
