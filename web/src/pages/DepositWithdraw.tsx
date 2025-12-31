import { useState } from 'react';
import {
  useCurrentAccount,
  useSignTransaction,
  useSuiClient,
} from '@mysten/dapp-kit';
import { useInventory } from '../hooks/useInventory';
import { InventoryCard } from '../components/InventoryCard';
import { CapacityBar, CapacityPreview } from '../components/CapacityBar';
import { ProofResult, ProofLoading, ProofError } from '../components/ProofResult';
import {
  OnChainInventorySelector,
  type LocalInventoryData,
} from '../components/OnChainInventorySelector';
import { useContractAddresses } from '../sui/ContractConfig';
import { buildWithdrawTx, buildDepositTx, buildBatchOperationsTx, hexToBytes, type BatchTxOperation } from '../sui/transactions';
import { ITEM_NAMES, ITEM_VOLUMES, canDeposit, calculateUsedVolume, getRegistryRoot } from '../types';
import * as api from '../api/client';
import type { BatchOperation, BatchOperationsResult } from '../api/client';
import type { StateTransitionResult } from '../types';
import type { OnChainInventory } from '../sui/hooks';
import { hasLocalSigner, getLocalAddress, signAndExecuteWithLocalSigner, getLocalnetClient } from '../sui/localSigner';

// Helper to fetch fresh inventory state from chain before proof generation
// This prevents stale nonce errors when inventory was modified elsewhere
async function fetchFreshInventory(
  inventoryId: string,
  useLocal: boolean
): Promise<OnChainInventory | null> {
  try {
    const client = useLocal ? getLocalnetClient() : null;
    if (!client) return null;

    const obj = await client.getObject({
      id: inventoryId,
      options: { showContent: true },
    });

    if (obj.data?.content?.dataType !== 'moveObject') {
      return null;
    }

    const fields = obj.data.content.fields as Record<string, unknown>;
    const commitmentBytes = fields.commitment as number[];
    const commitment = '0x' + commitmentBytes.map((b) => b.toString(16).padStart(2, '0')).join('');

    return {
      id: obj.data.objectId,
      commitment,
      owner: fields.owner as string,
      nonce: Number(fields.nonce),
      maxCapacity: Number(fields.max_capacity || 0),
    };
  } catch (error) {
    console.error('Failed to fetch fresh inventory:', error);
    return null;
  }
}

type Operation = 'deposit' | 'withdraw';
type Mode = 'demo' | 'onchain' | 'batch';

/** Pending operation to be included in batch */
interface PendingOperation {
  id: string;
  item_id: number;
  amount: number;
  op_type: 'deposit' | 'withdraw';
}

// Format gas cost in MIST to a readable string
function formatGasCost(mist: bigint): string {
  // 1 SUI = 1,000,000,000 MIST
  const sui = Number(mist) / 1_000_000_000;
  if (sui < 0.001) {
    return `${mist.toLocaleString()} MIST`;
  }
  return `${sui.toFixed(4)} SUI`;
}

export function DepositWithdraw() {
  const account = useCurrentAccount();
  const client = useSuiClient();
  const { packageId, verifyingKeysId, volumeRegistryId } = useContractAddresses();
  const { mutateAsync: signTransaction } = useSignTransaction();

  const useLocalSigner = hasLocalSigner();
  const localAddress = useLocalSigner ? getLocalAddress() : null;
  const effectiveAddress = localAddress || account?.address;

  const { inventory, generateBlinding, setSlots, setBlinding } = useInventory([
    { item_id: 1, quantity: 100 },
    { item_id: 2, quantity: 50 },
  ]);

  const [mode, setMode] = useState<Mode>('demo');
  const [selectedOnChainInventory, setSelectedOnChainInventory] =
    useState<OnChainInventory | null>(null);
  const [localData, setLocalData] = useState<LocalInventoryData | null>(null);

  const [operation, setOperation] = useState<Operation>('withdraw');
  const [itemId, setItemId] = useState(1);
  const [amount, setAmount] = useState(30);
  const [proofResult, setProofResult] = useState<StateTransitionResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [newInventory, setNewInventory] = useState<typeof inventory.slots | null>(null);
  const [txDigest, setTxDigest] = useState<string | null>(null);
  const [proofTimeMs, setProofTimeMs] = useState<number | null>(null);
  const [txTimeMs, setTxTimeMs] = useState<number | null>(null);
  const [gasCostMist, setGasCostMist] = useState<bigint | null>(null);

  // Batch mode state
  const [pendingOps, setPendingOps] = useState<PendingOperation[]>([]);
  const [batchResult, setBatchResult] = useState<BatchOperationsResult | null>(null);

  const currentSlots = mode === 'demo' ? inventory.slots : localData?.slots || [];
  const currentBlinding = mode === 'demo' ? inventory.blinding : localData?.blinding;
  const currentMaxCapacity = mode === 'demo' ? 0 : selectedOnChainInventory?.maxCapacity || 0;
  const hasCapacityLimit = currentMaxCapacity > 0 && volumeRegistryId?.startsWith('0x');

  const selectedItem = currentSlots.find((s) => s.item_id === itemId);
  const canWithdraw = selectedItem && selectedItem.quantity >= amount;
  const canDepositWithCapacity = !hasCapacityLimit || canDeposit(currentSlots, itemId, amount, currentMaxCapacity);

  const handleInventorySelect = (
    inv: OnChainInventory | null,
    data: LocalInventoryData | null
  ) => {
    setSelectedOnChainInventory(inv);
    setLocalData(data);
    setProofResult(null);
    setNewInventory(null);
    setTxDigest(null);
    setPendingOps([]);
    setBatchResult(null);
    if (data?.slots.length) {
      setItemId(data.slots[0].item_id);
    }
  };

  // Batch mode handlers
  const addToBatch = () => {
    const op: PendingOperation = {
      id: crypto.randomUUID(),
      item_id: itemId,
      amount: amount,
      op_type: operation,
    };
    setPendingOps([...pendingOps, op]);
  };

  const removeFromBatch = (id: string) => {
    setPendingOps(pendingOps.filter(op => op.id !== id));
  };

  const clearBatch = () => {
    setPendingOps([]);
    setBatchResult(null);
    setTxDigest(null);
    setError(null);
  };

  // Calculate preview of inventory after all batch operations
  const previewBatchInventory = () => {
    let preview = [...currentSlots];
    for (const op of pendingOps) {
      if (op.op_type === 'withdraw') {
        preview = preview
          .map(s => s.item_id === op.item_id ? { ...s, quantity: s.quantity - op.amount } : s)
          .filter(s => s.quantity > 0);
      } else {
        const idx = preview.findIndex(s => s.item_id === op.item_id);
        if (idx >= 0) {
          preview = preview.map(s => s.item_id === op.item_id ? { ...s, quantity: s.quantity + op.amount } : s);
        } else {
          preview = [...preview, { item_id: op.item_id, quantity: op.amount }];
        }
      }
    }
    return preview;
  };

  const executeBatch = async () => {
    if (!currentBlinding || !selectedOnChainInventory || !effectiveAddress || pendingOps.length === 0) {
      return;
    }

    setLoading(true);
    setError(null);
    setBatchResult(null);
    setTxDigest(null);
    setProofTimeMs(null);
    setTxTimeMs(null);
    setGasCostMist(null);

    try {
      // Fetch fresh inventory state
      const freshInventory = await fetchFreshInventory(selectedOnChainInventory.id, useLocalSigner);
      if (freshInventory) {
        setSelectedOnChainInventory(freshInventory);
      }
      const startNonce = freshInventory?.nonce ?? selectedOnChainInventory.nonce;

      const currentVolume = calculateUsedVolume(currentSlots);
      const registryRoot = getRegistryRoot();
      const maxCapacity = selectedOnChainInventory.maxCapacity;

      // Convert pending ops to batch operations
      const operations: BatchOperation[] = pendingOps.map(op => ({
        item_id: op.item_id,
        amount: op.amount,
        item_volume: ITEM_VOLUMES[op.item_id] ?? 0,
        op_type: op.op_type,
      }));

      // Generate all proofs in parallel
      const result = await api.proveBatchOperations(
        currentSlots,
        currentVolume,
        currentBlinding,
        operations,
        selectedOnChainInventory.id,
        startNonce,
        registryRoot,
        maxCapacity
      );

      setBatchResult(result);
      setProofTimeMs(result.proofTimeMs);

      // Build PTB with all operations
      const txOperations: BatchTxOperation[] = result.operations.map((opResult, i) => ({
        proof: hexToBytes(opResult.proof),
        signalHash: hexToBytes(opResult.public_inputs[0]),
        proofNonce: BigInt(opResult.nonce),
        proofInventoryId: hexToBytes(opResult.inventory_id),
        proofRegistryRoot: hexToBytes(opResult.registry_root),
        newCommitment: hexToBytes(opResult.new_commitment),
        itemId: pendingOps[i].item_id,
        amount: BigInt(pendingOps[i].amount),
        opType: pendingOps[i].op_type,
      }));

      const tx = buildBatchOperationsTx(
        packageId,
        selectedOnChainInventory.id,
        volumeRegistryId,
        verifyingKeysId,
        txOperations
      );

      // Execute transaction
      const txStart = performance.now();
      let txResult;

      if (useLocalSigner && localAddress) {
        console.log('Using local signer for batch operations:', localAddress);
        tx.setSender(localAddress);
        const localClient = getLocalnetClient();
        txResult = await signAndExecuteWithLocalSigner(tx, localClient);
      } else if (account) {
        tx.setSender(account.address);
        const signedTx = await signTransaction({
          transaction: tx as unknown as Parameters<typeof signTransaction>[0]['transaction'],
        });
        txResult = await client.executeTransactionBlock({
          transactionBlock: signedTx.bytes,
          signature: signedTx.signature,
          options: { showEffects: true },
        });
      } else {
        throw new Error('No signer available');
      }

      const txEnd = performance.now();
      setTxTimeMs(Math.round(txEnd - txStart));

      const effects = txResult.effects as {
        status?: { status: string; error?: string };
        gasUsed?: {
          computationCost: string;
          storageCost: string;
          storageRebate: string;
        };
      } | undefined;

      if (effects?.gasUsed) {
        const computation = BigInt(effects.gasUsed.computationCost);
        const storage = BigInt(effects.gasUsed.storageCost);
        const rebate = BigInt(effects.gasUsed.storageRebate);
        setGasCostMist(computation + storage - rebate);
      }

      if (effects?.status?.status === 'success') {
        setTxDigest(txResult.digest);

        // Update local storage with final state
        const stored = JSON.parse(localStorage.getItem('inventory-blindings') || '{}');
        stored[selectedOnChainInventory.id] = {
          blinding: result.finalBlinding,
          slots: result.finalInventory,
        };
        localStorage.setItem('inventory-blindings', JSON.stringify(stored));

        setLocalData({
          blinding: result.finalBlinding,
          slots: result.finalInventory,
        });

        // Clear pending operations
        setPendingOps([]);
      } else {
        throw new Error('Transaction failed: ' + effects?.status?.error);
      }
    } catch (err) {
      console.error('Batch operation error:', err);
      setError(err instanceof Error ? err.message : 'Failed to execute batch');
    } finally {
      setLoading(false);
    }
  };

  const handleOperation = async () => {
    if (!currentBlinding) {
      setError('No blinding factor available');
      return;
    }

    setLoading(true);
    setError(null);
    setProofResult(null);
    setNewInventory(null);
    setTxDigest(null);
    setProofTimeMs(null);
    setTxTimeMs(null);
    setGasCostMist(null);

    try {
      const newBlinding = await api.generateBlinding();
      const currentVolume = calculateUsedVolume(currentSlots);
      const itemVolume = ITEM_VOLUMES[itemId] ?? 0;
      // Get registry root hash for volume validation
      const registryRoot = getRegistryRoot();

      // For on-chain operations, fetch fresh inventory state to get current nonce
      // This prevents stale nonce errors if inventory was modified elsewhere
      let freshInventory = selectedOnChainInventory;
      if (mode === 'onchain' && selectedOnChainInventory) {
        const fetched = await fetchFreshInventory(selectedOnChainInventory.id, useLocalSigner);
        if (fetched) {
          freshInventory = fetched;
          // Update the selected inventory state with fresh data
          setSelectedOnChainInventory(fetched);
        }
      }

      // Get nonce and inventory_id for security binding
      const currentNonce = mode === 'onchain' && freshInventory
        ? freshInventory.nonce
        : 0;
      const currentInventoryId = mode === 'onchain' && freshInventory
        ? freshInventory.id
        : '0x0000000000000000000000000000000000000000000000000000000000000000';

      let updatedSlots: typeof currentSlots;

      // Use the unified state transition API with timing
      const proofStart = performance.now();
      const result = await api.proveStateTransition({
        inventory: currentSlots,
        current_volume: currentVolume,
        old_blinding: currentBlinding,
        new_blinding: newBlinding,
        item_id: itemId,
        amount: amount,
        item_volume: itemVolume,
        registry_root: registryRoot,
        max_capacity: currentMaxCapacity,
        nonce: currentNonce,
        inventory_id: currentInventoryId,
        op_type: operation,
      });
      const proofEnd = performance.now();
      setProofTimeMs(Math.round(proofEnd - proofStart));

      if (operation === 'withdraw') {
        updatedSlots = currentSlots
          .map((s) =>
            s.item_id === itemId ? { ...s, quantity: s.quantity - amount } : s
          )
          .filter((s) => s.quantity > 0);
      } else {
        const existingIndex = currentSlots.findIndex((s) => s.item_id === itemId);
        if (existingIndex >= 0) {
          updatedSlots = currentSlots.map((s) =>
            s.item_id === itemId ? { ...s, quantity: s.quantity + amount } : s
          );
        } else {
          updatedSlots = [...currentSlots, { item_id: itemId, quantity: amount }];
        }
      }

      setProofResult(result);
      setNewInventory(updatedSlots);

      if (mode === 'onchain' && selectedOnChainInventory && effectiveAddress) {
        await executeOnChain(result, newBlinding, updatedSlots);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to generate proof');
    } finally {
      setLoading(false);
    }
  };

  const executeOnChain = async (
    result: StateTransitionResult,
    newBlinding: string,
    updatedSlots: typeof currentSlots
  ) => {
    if (!selectedOnChainInventory || !effectiveAddress || !volumeRegistryId) return;

    const txStart = performance.now();
    try {
      const proofBytes = hexToBytes(result.proof);
      const signalHashBytes = hexToBytes(result.public_inputs[0]);
      const newCommitmentBytes = hexToBytes(result.new_commitment);
      // Security parameters from proof result
      const proofInventoryIdBytes = hexToBytes(result.inventory_id);
      const proofRegistryRootBytes = hexToBytes(result.registry_root);

      let tx;
      if (operation === 'withdraw') {
        tx = buildWithdrawTx(
          packageId,
          selectedOnChainInventory.id,
          volumeRegistryId,
          verifyingKeysId,
          proofBytes,
          signalHashBytes,
          BigInt(result.nonce),
          proofInventoryIdBytes,
          proofRegistryRootBytes,
          newCommitmentBytes,
          itemId,
          BigInt(amount)
        );
      } else {
        // Both deposit and deposit with capacity now use the same unified function
        tx = buildDepositTx(
          packageId,
          selectedOnChainInventory.id,
          volumeRegistryId,
          verifyingKeysId,
          proofBytes,
          signalHashBytes,
          BigInt(result.nonce),
          proofInventoryIdBytes,
          proofRegistryRootBytes,
          newCommitmentBytes,
          itemId,
          BigInt(amount)
        );
      }

      let txResult;

      if (useLocalSigner && localAddress) {
        console.log('Using local signer for deposit/withdraw:', localAddress);
        tx.setSender(localAddress);
        const localClient = getLocalnetClient();
        txResult = await signAndExecuteWithLocalSigner(tx, localClient);
      } else if (account) {
        tx.setSender(account.address);
        const signedTx = await signTransaction({
          transaction: tx as unknown as Parameters<typeof signTransaction>[0]['transaction'],
        });
        txResult = await client.executeTransactionBlock({
          transactionBlock: signedTx.bytes,
          signature: signedTx.signature,
          options: { showEffects: true },
        });
      } else {
        throw new Error('No signer available');
      }

      const txEnd = performance.now();
      setTxTimeMs(Math.round(txEnd - txStart));

      const effects = txResult.effects as {
        status?: { status: string; error?: string };
        gasUsed?: {
          computationCost: string;
          storageCost: string;
          storageRebate: string;
        };
      } | undefined;

      // Calculate total gas cost in MIST
      if (effects?.gasUsed) {
        const computation = BigInt(effects.gasUsed.computationCost);
        const storage = BigInt(effects.gasUsed.storageCost);
        const rebate = BigInt(effects.gasUsed.storageRebate);
        setGasCostMist(computation + storage - rebate);
      }

      if (effects?.status?.status === 'success') {
        setTxDigest(txResult.digest);

        const stored = JSON.parse(localStorage.getItem('inventory-blindings') || '{}');
        stored[selectedOnChainInventory.id] = {
          blinding: newBlinding,
          slots: updatedSlots,
        };
        localStorage.setItem('inventory-blindings', JSON.stringify(stored));

        setLocalData({
          blinding: newBlinding,
          slots: updatedSlots,
        });
      } else {
        throw new Error('Transaction failed: ' + effects?.status?.error);
      }
    } catch (err) {
      console.error('On-chain execution error:', err);
      setError(
        `Proof generated but on-chain execution failed: ${
          err instanceof Error ? err.message : 'Unknown error'
        }`
      );
    }
  };

  const loadSampleInventory = async () => {
    setSlots([
      { item_id: 1, quantity: 100 },
      { item_id: 2, quantity: 50 },
    ]);
    await generateBlinding();
    setProofResult(null);
    setNewInventory(null);
    setTxDigest(null);
  };

  const applyChanges = async () => {
    if (newInventory && proofResult) {
      setSlots(newInventory);
      const newBlinding = await api.generateBlinding();
      setBlinding(newBlinding);
      setProofResult(null);
      setNewInventory(null);
    }
  };

  return (
    <div className="col">
      <div className="mb-2">
        <h1>DEPOSIT / WITHDRAW</h1>
        <p className="text-muted">
          Prove valid state transitions when adding or removing items.
        </p>
      </div>

      {/* Mode Toggle */}
      <div className="btn-group mb-2">
        <button
          onClick={() => {
            setMode('demo');
            setProofResult(null);
            setNewInventory(null);
            setTxDigest(null);
            setPendingOps([]);
            setBatchResult(null);
          }}
          className={`btn btn-secondary ${mode === 'demo' ? 'active' : ''}`}
        >
          [DEMO]
        </button>
        <button
          onClick={() => {
            setMode('onchain');
            setProofResult(null);
            setNewInventory(null);
            setTxDigest(null);
            setPendingOps([]);
            setBatchResult(null);
          }}
          className={`btn btn-secondary ${mode === 'onchain' ? 'active' : ''}`}
        >
          [ON-CHAIN]
        </button>
        <button
          onClick={() => {
            setMode('batch');
            setProofResult(null);
            setNewInventory(null);
            setTxDigest(null);
            setBatchResult(null);
          }}
          className={`btn btn-secondary ${mode === 'batch' ? 'active' : ''}`}
        >
          [BATCH]
        </button>
      </div>

      <div className="grid grid-2">
        {/* Left: Configuration */}
        <div className="col">
          {mode === 'demo' ? (
            <div className="card">
              <div className="card-header">
                <div className="card-header-left"></div>
                <span className="card-title">DEMO INVENTORY</span>
                <div className="card-header-right"></div>
              </div>
              <div className="card-body">
                <div className="row-between mb-2">
                  <span className="text-small text-muted">SAMPLE DATA</span>
                  <button onClick={loadSampleInventory} className="btn btn-secondary btn-small">
                    [RESET]
                  </button>
                </div>

                <InventoryCard
                  title=""
                  slots={inventory.slots}
                  commitment={inventory.commitment}
                  blinding={inventory.blinding}
                  showBlinding={false}
                />

                {!inventory.blinding && (
                  <button onClick={generateBlinding} className="btn btn-primary mt-2" style={{ width: '100%' }}>
                    [GENERATE BLINDING]
                  </button>
                )}
              </div>
            </div>
          ) : (
            <div className="card">
              <div className="card-header">
                <div className="card-header-left"></div>
                <span className="card-title">{mode === 'batch' ? 'SELECT INVENTORY' : 'ON-CHAIN INVENTORY'}</span>
                <div className="card-header-right"></div>
              </div>
              <div className="card-body">
                <OnChainInventorySelector
                  selectedInventory={selectedOnChainInventory}
                  onSelect={handleInventorySelect}
                />
              </div>
            </div>
          )}

          <div className="card">
            <div className="card-header">
              <div className="card-header-left"></div>
              <span className="card-title">OPERATION</span>
              <div className="card-header-right"></div>
            </div>
            <div className="card-body">
              <div className="btn-group mb-2" style={{ width: '100%' }}>
                <button
                  onClick={() => setOperation('withdraw')}
                  className={`btn btn-secondary ${operation === 'withdraw' ? 'active' : ''}`}
                  style={{ flex: 1 }}
                >
                  [WITHDRAW]
                </button>
                <button
                  onClick={() => setOperation('deposit')}
                  className={`btn btn-secondary ${operation === 'deposit' ? 'active' : ''}`}
                  style={{ flex: 1 }}
                >
                  [DEPOSIT]
                </button>
              </div>

              <div className="input-group">
                <label className="input-label">Item</label>
                <select
                  value={itemId}
                  onChange={(e) => setItemId(Number(e.target.value))}
                  className="select"
                >
                  {operation === 'withdraw' ? (
                    currentSlots.length === 0 ? (
                      <option>No items available</option>
                    ) : (
                      currentSlots.map((slot) => (
                        <option key={slot.item_id} value={slot.item_id}>
                          {ITEM_NAMES[slot.item_id] || `Item #${slot.item_id}`} (have {slot.quantity})
                        </option>
                      ))
                    )
                  ) : (
                    Object.entries(ITEM_NAMES).map(([id, name]) => (
                      <option key={id} value={id}>
                        {name} (#{id})
                      </option>
                    ))
                  )}
                </select>
              </div>

              <div className="input-group">
                <label className="input-label">Amount</label>
                <input
                  type="number"
                  value={amount}
                  onChange={(e) => setAmount(Number(e.target.value))}
                  min={1}
                  className="input"
                />
                {operation === 'withdraw' && selectedItem && (
                  <p className={`text-small mt-1 ${canWithdraw ? 'text-success' : 'text-error'}`}>
                    {canWithdraw
                      ? `[OK] Withdrawing ${amount} of ${selectedItem.quantity}`
                      : `[!!] Insufficient (have ${selectedItem.quantity})`}
                  </p>
                )}
                {operation === 'deposit' && (
                  <p className="text-small text-muted mt-1">
                    Volume: {ITEM_VOLUMES[itemId] ?? 0} x {amount} = {(ITEM_VOLUMES[itemId] ?? 0) * amount}
                  </p>
                )}
              </div>

              {mode === 'onchain' && selectedOnChainInventory && currentMaxCapacity > 0 && (
                <div className="col">
                  <CapacityBar slots={currentSlots} maxCapacity={currentMaxCapacity} />
                  {operation === 'deposit' && (
                    <CapacityPreview
                      currentSlots={currentSlots}
                      maxCapacity={currentMaxCapacity}
                      itemId={itemId}
                      amount={amount}
                      isDeposit={true}
                    />
                  )}
                </div>
              )}

              {operation === 'deposit' && !canDepositWithCapacity && (
                <div className="alert alert-error">
                  [!!] Deposit would exceed inventory capacity!
                </div>
              )}

              {mode === 'batch' ? (
                <button
                  onClick={addToBatch}
                  disabled={
                    !currentBlinding ||
                    !selectedOnChainInventory ||
                    (operation === 'withdraw' && !canWithdraw) ||
                    (operation === 'deposit' && !canDepositWithCapacity)
                  }
                  className="btn btn-primary"
                  style={{ width: '100%' }}
                >
                  [+ ADD TO BATCH]
                </button>
              ) : (
                <button
                  onClick={handleOperation}
                  disabled={
                    loading ||
                    !currentBlinding ||
                    (operation === 'withdraw' && !canWithdraw) ||
                    (operation === 'deposit' && !canDepositWithCapacity)
                  }
                  className={`btn ${operation === 'withdraw' ? 'btn-danger' : 'btn-success'}`}
                  style={{ width: '100%' }}
                >
                  {loading
                    ? 'PROCESSING...'
                    : mode === 'onchain'
                    ? `[${operation.toUpperCase()} ON-CHAIN]`
                    : `[${operation.toUpperCase()} ${amount}]`}
                </button>
              )}
            </div>
          </div>

          {/* Batch: Pending Operations */}
          {mode === 'batch' && selectedOnChainInventory && localData && (
            <div className="card">
              <div className="card-header">
                <div className="card-header-left"></div>
                <span className="card-title">PENDING OPERATIONS ({pendingOps.length})</span>
                <div className="card-header-right">
                  {pendingOps.length > 0 && (
                    <button onClick={clearBatch} className="btn btn-secondary btn-small">
                      [CLEAR]
                    </button>
                  )}
                </div>
              </div>
              <div className="card-body">
                {pendingOps.length === 0 ? (
                  <div className="text-muted text-center">No operations queued</div>
                ) : (
                  <div className="col">
                    {pendingOps.map((op) => (
                      <div key={op.id} className="row-between" style={{ padding: '0.5rem', background: 'var(--bg-secondary)', marginBottom: '0.5rem' }}>
                        <span>
                          <span className={op.op_type === 'withdraw' ? 'text-error' : 'text-success'}>
                            {op.op_type === 'withdraw' ? '[-]' : '[+]'}
                          </span>{' '}
                          {op.amount} {ITEM_NAMES[op.item_id] || `#${op.item_id}`}
                        </span>
                        <button
                          onClick={() => removeFromBatch(op.id)}
                          className="btn btn-secondary btn-small"
                        >
                          [X]
                        </button>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </div>
          )}
        </div>

        {/* Right: Results */}
        <div className="col">
          {/* Batch Mode: Preview & Execute */}
          {mode === 'batch' && selectedOnChainInventory && localData && (
            <div className="card">
              <div className="card-header">
                <div className="card-header-left"></div>
                <span className="card-title">PREVIEW</span>
                <div className="card-header-right"></div>
              </div>
              <div className="card-body">
                <div className="grid grid-2">
                  <div>
                    <div className="text-small text-muted mb-1">CURRENT</div>
                    <div className="col text-small">
                      {currentSlots.map(s => (
                        <div key={s.item_id}>
                          {ITEM_NAMES[s.item_id] || `#${s.item_id}`}: {s.quantity}
                        </div>
                      ))}
                      {currentSlots.length === 0 && <div className="text-muted">Empty</div>}
                    </div>
                  </div>
                  <div>
                    <div className="text-small text-muted mb-1">AFTER BATCH</div>
                    <div className="col text-small">
                      {previewBatchInventory().map(s => (
                        <div key={s.item_id}>
                          {ITEM_NAMES[s.item_id] || `#${s.item_id}`}: {s.quantity}
                        </div>
                      ))}
                      {previewBatchInventory().length === 0 && <div className="text-muted">Empty</div>}
                    </div>
                  </div>
                </div>

                <button
                  onClick={executeBatch}
                  disabled={loading || pendingOps.length === 0 || !currentBlinding}
                  className="btn btn-primary mt-2"
                  style={{ width: '100%' }}
                >
                  {loading
                    ? 'PROCESSING...'
                    : `[EXECUTE ${pendingOps.length} OPERATION${pendingOps.length !== 1 ? 'S' : ''}]`}
                </button>
              </div>
            </div>
          )}

          {/* Batch Mode: Success Result */}
          {mode === 'batch' && txDigest && batchResult && (
            <div className="alert alert-success">
              <div className="row-between">
                <span>[OK] BATCH OPERATION SUCCESSFUL</span>
                <span className="text-small">
                  <span className="badge">{batchResult.proofTimeMs}ms proofs</span>
                  {txTimeMs !== null && <span className="badge" style={{ marginLeft: '0.5ch' }}>{txTimeMs}ms tx</span>}
                  {gasCostMist !== null && <span className="badge" style={{ marginLeft: '0.5ch' }}>{formatGasCost(gasCostMist)}</span>}
                </span>
              </div>
              <div className="text-small mt-1">
                {batchResult.operations.length} operations executed atomically on Sui blockchain.
              </div>
              <code className="text-break text-small">{txDigest}</code>
            </div>
          )}

          {/* Batch Mode: Proof Details */}
          {mode === 'batch' && batchResult && (
            <div className="card">
              <div className="card-header">
                <div className="card-header-left"></div>
                <span className="card-title">PROOF DETAILS</span>
                <div className="card-header-right"></div>
              </div>
              <div className="card-body">
                <div className="text-small text-success mb-2">
                  [OK] Generated {batchResult.operations.length} proofs in parallel ({batchResult.proofTimeMs}ms wall-clock)
                </div>
                <div className="col">
                  {batchResult.operations.map((op, i) => (
                    <div key={i} className="card-simple mb-1">
                      <div className="row-between">
                        <span className="text-small">
                          Proof #{i + 1} (nonce {op.nonce})
                        </span>
                        <code className="text-small">{op.proof.slice(0, 20)}...</code>
                      </div>
                    </div>
                  ))}
                </div>
                <div className="mt-2">
                  <div className="text-small text-muted">FINAL COMMITMENT</div>
                  <code className="text-break text-small">{batchResult.finalCommitment}</code>
                </div>
              </div>
            </div>
          )}

          {/* Batch Mode: How It Works */}
          {mode === 'batch' && !batchResult && !error && selectedOnChainInventory && (
            <div className="card">
              <div className="card-header">
                <div className="card-header-left"></div>
                <span className="card-title">HOW IT WORKS</span>
                <div className="card-header-right"></div>
              </div>
              <div className="card-body">
                <div className="col text-small">
                  <div>[1] Queue multiple operations (deposits/withdraws)</div>
                  <div>[2] Pre-compute intermediate states locally</div>
                  <div>[3] Generate all proofs IN PARALLEL (~450ms total)</div>
                  <div>[4] Submit single atomic transaction (PTB)</div>
                  <div>[5] All operations verified and applied atomically</div>
                </div>
                <div className="text-small text-muted mt-2">
                  Parallel proof generation means wall-clock time is O(1) regardless of N operations!
                </div>
              </div>
            </div>
          )}

          {/* Non-Batch Mode: Success Result */}
          {mode !== 'batch' && txDigest && (
            <div className="alert alert-success">
              <div className="row-between">
                <span>[OK] ON-CHAIN {operation.toUpperCase()} SUCCESSFUL</span>
                {(proofTimeMs !== null || txTimeMs !== null || gasCostMist !== null) && (
                  <span className="text-small">
                    {proofTimeMs !== null && <span className="badge">{proofTimeMs}ms proof</span>}
                    {txTimeMs !== null && <span className="badge" style={{ marginLeft: '0.5ch' }}>{txTimeMs}ms tx</span>}
                    {gasCostMist !== null && <span className="badge" style={{ marginLeft: '0.5ch' }}>{formatGasCost(gasCostMist)}</span>}
                  </span>
                )}
              </div>
              <div className="text-small mt-1">Transaction executed on Sui blockchain.</div>
              <code className="text-break text-small">{txDigest}</code>
            </div>
          )}

          {/* Non-batch mode content */}
          {mode !== 'batch' && (newInventory || proofResult) && (
            <div className="card">
              <div className="card-header">
                <div className="card-header-left"></div>
                <span className="card-title">STATE TRANSITION</span>
                <div className="card-header-right"></div>
              </div>
              <div className="card-body">
                <div className="row" style={{ alignItems: 'stretch' }}>
                  <div style={{ flex: 1 }}>
                    <div className="text-small text-muted mb-1">BEFORE</div>
                    <div className="badge">
                      {currentSlots
                        .map((s) => `${ITEM_NAMES[s.item_id] || `#${s.item_id}`}: ${s.quantity}`)
                        .join(', ') || 'Empty'}
                    </div>
                  </div>

                  <div className="text-muted" style={{ padding: '0 1ch' }}>-&gt;</div>

                  <div style={{ flex: 1 }}>
                    <div className="text-small text-muted mb-1">AFTER</div>
                    <div className="badge badge-success">
                      {newInventory
                        ?.map((s) => `${ITEM_NAMES[s.item_id] || `#${s.item_id}`}: ${s.quantity}`)
                        .join(', ') || 'Empty'}
                    </div>
                  </div>
                </div>

                {proofResult && (
                  <div className="mt-2">
                    <div className="text-small text-muted mb-1">NEW COMMITMENT</div>
                    <code className="text-break text-small">{proofResult.new_commitment}</code>
                  </div>
                )}

                {mode === 'demo' && proofResult && (
                  <button onClick={applyChanges} className="btn btn-primary mt-2" style={{ width: '100%' }}>
                    [APPLY CHANGES]
                  </button>
                )}
              </div>
            </div>
          )}

          {loading && (
            <ProofLoading
              message={mode === 'batch'
                ? `Executing ${pendingOps.length} operations...`
                : `${mode === 'onchain' ? 'Executing' : 'Generating'} ${operation} ${mode === 'onchain' ? 'on-chain' : 'proof'}...`
              }
            />
          )}

          {error && <ProofError error={error} onRetry={mode === 'batch' ? executeBatch : handleOperation} />}

          {mode !== 'batch' && proofResult && (
            <ProofResult
              result={proofResult}
              title={`${operation === 'withdraw' ? 'Withdrawal' : 'Deposit'} Proof`}
              extra={
                <div className="row-between">
                  <span className="text-small text-success">
                    [OK] Proved valid {operation} of <strong>{amount}</strong>{' '}
                    <strong>{ITEM_NAMES[itemId] || `Item #${itemId}`}</strong>.
                  </span>
                  {proofTimeMs !== null && (
                    <span className="badge">{proofTimeMs}ms</span>
                  )}
                </div>
              }
            />
          )}

          {mode !== 'batch' && !loading && !proofResult && !error && (
            <div className="card">
              <div className="card-header">
                <div className="card-header-left"></div>
                <span className="card-title">WHAT GETS PROVEN</span>
                <div className="card-header-right"></div>
              </div>
              <div className="card-body">
                <div className="col text-small">
                  <div>[OK] Old commitment matches your claimed inventory</div>
                  <div>[OK] {operation === 'withdraw' ? 'Sufficient balance exists for withdrawal' : 'New item was added correctly'}</div>
                  <div>[OK] New commitment is correctly computed</div>
                  <div>[OK] No other items were modified</div>
                  {mode === 'onchain' && (
                    <div>[OK] Commitment is updated on-chain via ZK proof verification</div>
                  )}
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
