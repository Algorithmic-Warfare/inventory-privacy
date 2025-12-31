import { useState } from 'react';
import {
  useCurrentAccount,
  useSignTransaction,
  useSuiClient,
} from '@mysten/dapp-kit';
import { InventoryCard } from '../components/InventoryCard';
import { CapacityBar, CapacityPreview } from '../components/CapacityBar';
import { ProofResult, ProofLoading, ProofError } from '../components/ProofResult';
import {
  OnChainInventorySelector,
  type LocalInventoryData,
} from '../components/OnChainInventorySelector';
import { useContractAddresses } from '../sui/ContractConfig';
import { buildTransferTx, buildBatchTransfersTx, hexToBytes, type BatchTransferTxOperation } from '../sui/transactions';
import { ITEM_NAMES, ITEM_VOLUMES, canDeposit, calculateUsedVolume, getRegistryRoot, type InventorySlot } from '../types';
import * as api from '../api/client';
import type { TransferProofs } from '../api/client';

type TransferResult = TransferProofs;
import type { OnChainInventory } from '../sui/hooks';
import { hasLocalSigner, getLocalAddress, signAndExecuteWithLocalSigner, getLocalnetClient } from '../sui/localSigner';

/** Pending transfer for batch mode */
interface PendingTransfer {
  id: string;
  item_id: number;
  amount: number;
}

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

// Format gas cost in MIST to a readable string
function formatGasCost(mist: bigint): string {
  // 1 SUI = 1,000,000,000 MIST
  const sui = Number(mist) / 1_000_000_000;
  if (sui < 0.001) {
    return `${mist.toLocaleString()} MIST`;
  }
  return `${sui.toFixed(4)} SUI`;
}

interface InventoryState {
  slots: InventorySlot[];
  blinding: string;
  commitment: string | null;
}

type Mode = 'demo' | 'onchain' | 'batch';

/** Batch transfer result */
interface BatchTransferResult {
  transfers: TransferResult[];
  finalSrcSlots: InventorySlot[];
  finalDstSlots: InventorySlot[];
  finalSrcBlinding: string;
  finalDstBlinding: string;
  proofTimeMs: number;
}

export function Transfer() {
  const account = useCurrentAccount();
  const client = useSuiClient();
  const { packageId, verifyingKeysId, volumeRegistryId } = useContractAddresses();
  const { mutateAsync: signTransaction } = useSignTransaction();

  const useLocalSigner = hasLocalSigner();
  const localAddress = useLocalSigner ? getLocalAddress() : null;
  const effectiveAddress = localAddress || account?.address;

  const [mode, setMode] = useState<Mode>('demo');

  const [source, setSource] = useState<InventoryState>({
    slots: [
      { item_id: 1, quantity: 100 },
      { item_id: 2, quantity: 50 },
    ],
    blinding: '',
    commitment: null,
  });

  const [destination, setDestination] = useState<InventoryState>({
    slots: [{ item_id: 3, quantity: 25 }],
    blinding: '',
    commitment: null,
  });

  const [srcOnChain, setSrcOnChain] = useState<OnChainInventory | null>(null);
  const [srcLocalData, setSrcLocalData] = useState<LocalInventoryData | null>(null);
  const [dstOnChain, setDstOnChain] = useState<OnChainInventory | null>(null);
  const [dstLocalData, setDstLocalData] = useState<LocalInventoryData | null>(null);

  const [itemId, setItemId] = useState(1);
  const [amount, setAmount] = useState(30);
  const [proofResult, setProofResult] = useState<TransferResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [transferComplete, setTransferComplete] = useState(false);
  const [txDigest, setTxDigest] = useState<string | null>(null);
  const [proofTimeMs, setProofTimeMs] = useState<number | null>(null);
  const [txTimeMs, setTxTimeMs] = useState<number | null>(null);
  const [gasCostMist, setGasCostMist] = useState<bigint | null>(null);

  // Batch mode state
  const [pendingTransfers, setPendingTransfers] = useState<PendingTransfer[]>([]);
  const [batchResult, setBatchResult] = useState<BatchTransferResult | null>(null);

  const currentSrcSlots = mode === 'demo' ? source.slots : srcLocalData?.slots || [];
  const currentDstSlots = mode === 'demo' ? destination.slots : dstLocalData?.slots || [];
  const currentSrcBlinding = mode === 'demo' ? source.blinding : srcLocalData?.blinding;
  const currentDstBlinding = mode === 'demo' ? destination.blinding : dstLocalData?.blinding;

  const sourceItem = currentSrcSlots.find((s) => s.item_id === itemId);
  const canTransfer = sourceItem && sourceItem.quantity >= amount;

  const dstMaxCapacity = mode === 'demo' ? 0 : dstOnChain?.maxCapacity || 0;
  const hasDstCapacityLimit = dstMaxCapacity > 0 && volumeRegistryId?.startsWith('0x');
  const canTransferWithCapacity = !hasDstCapacityLimit || canDeposit(currentDstSlots, itemId, amount, dstMaxCapacity);

  const initializeBlindings = async () => {
    const [srcBlinding, dstBlinding] = await Promise.all([
      api.generateBlinding(),
      api.generateBlinding(),
    ]);

    const srcVolume = calculateUsedVolume(source.slots);
    const dstVolume = calculateUsedVolume(destination.slots);

    const [srcCommitmentResult, dstCommitmentResult] = await Promise.all([
      api.createCommitment(source.slots, srcVolume, srcBlinding),
      api.createCommitment(destination.slots, dstVolume, dstBlinding),
    ]);

    setSource((prev) => ({
      ...prev,
      blinding: srcBlinding,
      commitment: srcCommitmentResult.commitment,
    }));
    setDestination((prev) => ({
      ...prev,
      blinding: dstBlinding,
      commitment: dstCommitmentResult.commitment,
    }));
  };

  const handleTransfer = async () => {
    if (!currentSrcBlinding || !currentDstBlinding) {
      setError('Both inventories must have blinding factors');
      return;
    }

    setLoading(true);
    setError(null);
    setProofResult(null);
    setTransferComplete(false);
    setTxDigest(null);
    setProofTimeMs(null);
    setTxTimeMs(null);
    setGasCostMist(null);

    try {
      const [srcNewBlinding, dstNewBlinding] = await Promise.all([
        api.generateBlinding(),
        api.generateBlinding(),
      ]);

      const srcVolume = calculateUsedVolume(currentSrcSlots);
      const dstVolume = calculateUsedVolume(currentDstSlots);
      const itemVolume = ITEM_VOLUMES[itemId] ?? 0;
      const registryRoot = getRegistryRoot();
      const srcMaxCapacity = mode === 'demo' ? 0 : srcOnChain?.maxCapacity || 0;

      // For on-chain operations, fetch fresh inventory state to get current nonces
      // This prevents stale nonce errors if inventories were modified elsewhere
      let freshSrcOnChain = srcOnChain;
      let freshDstOnChain = dstOnChain;
      if (mode === 'onchain') {
        const [fetchedSrc, fetchedDst] = await Promise.all([
          srcOnChain ? fetchFreshInventory(srcOnChain.id, useLocalSigner) : null,
          dstOnChain ? fetchFreshInventory(dstOnChain.id, useLocalSigner) : null,
        ]);
        if (fetchedSrc) {
          freshSrcOnChain = fetchedSrc;
          setSrcOnChain(fetchedSrc);
        }
        if (fetchedDst) {
          freshDstOnChain = fetchedDst;
          setDstOnChain(fetchedDst);
        }
      }

      // Get nonce and inventory_id for security binding (using fresh data)
      const srcNonce = mode === 'onchain' && freshSrcOnChain ? freshSrcOnChain.nonce : 0;
      const srcInventoryId = mode === 'onchain' && freshSrcOnChain
        ? freshSrcOnChain.id
        : '0x0000000000000000000000000000000000000000000000000000000000000000';
      const dstNonce = mode === 'onchain' && freshDstOnChain ? freshDstOnChain.nonce : 0;
      const dstInventoryId = mode === 'onchain' && freshDstOnChain
        ? freshDstOnChain.id
        : '0x0000000000000000000000000000000000000000000000000000000000000000';

      const proofStart = performance.now();
      const result = await api.proveTransfer(
        currentSrcSlots,
        srcVolume,
        currentSrcBlinding,
        srcNewBlinding,
        srcNonce,
        srcInventoryId,
        currentDstSlots,
        dstVolume,
        currentDstBlinding,
        dstNewBlinding,
        dstNonce,
        dstInventoryId,
        itemId,
        amount,
        itemVolume,
        registryRoot,
        srcMaxCapacity,
        dstMaxCapacity
      );
      const proofEnd = performance.now();
      setProofTimeMs(Math.round(proofEnd - proofStart));

      setProofResult(result);

      const newSourceSlots = currentSrcSlots
        .map((s) =>
          s.item_id === itemId ? { ...s, quantity: s.quantity - amount } : s
        )
        .filter((s) => s.quantity > 0);

      const existingDstIndex = currentDstSlots.findIndex((s) => s.item_id === itemId);
      let newDstSlots: InventorySlot[];
      if (existingDstIndex >= 0) {
        newDstSlots = currentDstSlots.map((s) =>
          s.item_id === itemId ? { ...s, quantity: s.quantity + amount } : s
        );
      } else {
        newDstSlots = [...currentDstSlots, { item_id: itemId, quantity: amount }];
      }

      if (mode === 'demo') {
        setSource({
          slots: newSourceSlots,
          blinding: srcNewBlinding,
          commitment: result.srcNewCommitment,
        });

        setDestination({
          slots: newDstSlots,
          blinding: dstNewBlinding,
          commitment: result.dstNewCommitment,
        });

        setTransferComplete(true);
      } else if (srcOnChain && dstOnChain && effectiveAddress) {
        await executeOnChain(
          result,
          srcNewBlinding,
          dstNewBlinding,
          newSourceSlots,
          newDstSlots
        );
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to generate proof');
    } finally {
      setLoading(false);
    }
  };

  const executeOnChain = async (
    result: TransferResult,
    srcNewBlinding: string,
    dstNewBlinding: string,
    newSourceSlots: InventorySlot[],
    newDstSlots: InventorySlot[]
  ) => {
    if (!srcOnChain || !dstOnChain || !effectiveAddress || !volumeRegistryId) return;

    const txStart = performance.now();
    try {
      const srcProofBytes = hexToBytes(result.srcProof.proof);
      const srcSignalHashBytes = hexToBytes(result.srcProof.public_inputs[0]);
      const srcNewCommitmentBytes = hexToBytes(result.srcNewCommitment);
      const srcInventoryIdBytes = hexToBytes(result.srcInventoryId);
      const srcRegistryRootBytes = hexToBytes(result.srcRegistryRoot);
      const dstProofBytes = hexToBytes(result.dstProof.proof);
      const dstSignalHashBytes = hexToBytes(result.dstProof.public_inputs[0]);
      const dstNewCommitmentBytes = hexToBytes(result.dstNewCommitment);
      const dstInventoryIdBytes = hexToBytes(result.dstInventoryId);
      const dstRegistryRootBytes = hexToBytes(result.dstRegistryRoot);

      const tx = buildTransferTx(
        packageId,
        srcOnChain.id,
        dstOnChain.id,
        volumeRegistryId,
        verifyingKeysId,
        // Source parameters
        srcProofBytes,
        srcSignalHashBytes,
        BigInt(result.srcNonce),
        srcInventoryIdBytes,
        srcRegistryRootBytes,
        srcNewCommitmentBytes,
        // Destination parameters
        dstProofBytes,
        dstSignalHashBytes,
        BigInt(result.dstNonce),
        dstInventoryIdBytes,
        dstRegistryRootBytes,
        dstNewCommitmentBytes,
        // Transfer metadata
        itemId,
        BigInt(amount)
      );

      let txResult;

      if (useLocalSigner && localAddress) {
        console.log('Using local signer for transfer:', localAddress);
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
        setTransferComplete(true);

        const stored = JSON.parse(localStorage.getItem('inventory-blindings') || '{}');
        stored[srcOnChain.id] = {
          blinding: srcNewBlinding,
          slots: newSourceSlots,
        };
        stored[dstOnChain.id] = {
          blinding: dstNewBlinding,
          slots: newDstSlots,
        };
        localStorage.setItem('inventory-blindings', JSON.stringify(stored));

        setSrcLocalData({
          blinding: srcNewBlinding,
          slots: newSourceSlots,
        });
        setDstLocalData({
          blinding: dstNewBlinding,
          slots: newDstSlots,
        });
      } else {
        throw new Error('Transaction failed: ' + effects?.status?.error);
      }
    } catch (err) {
      console.error('On-chain transfer error:', err);
      setError(
        `Proof generated but on-chain transfer failed: ${
          err instanceof Error ? err.message : 'Unknown error'
        }`
      );
    }
  };

  const resetDemo = async () => {
    setSource({
      slots: [
        { item_id: 1, quantity: 100 },
        { item_id: 2, quantity: 50 },
      ],
      blinding: '',
      commitment: null,
    });
    setDestination({
      slots: [{ item_id: 3, quantity: 25 }],
      blinding: '',
      commitment: null,
    });
    setProofResult(null);
    setError(null);
    setTransferComplete(false);
    setTxDigest(null);
  };

  // Batch mode handlers
  const addToBatch = () => {
    const transfer: PendingTransfer = {
      id: crypto.randomUUID(),
      item_id: itemId,
      amount: amount,
    };
    setPendingTransfers([...pendingTransfers, transfer]);
  };

  const removeFromBatch = (id: string) => {
    setPendingTransfers(pendingTransfers.filter(t => t.id !== id));
  };

  const clearBatch = () => {
    setPendingTransfers([]);
    setBatchResult(null);
    setError(null);
    setTxDigest(null);
  };

  // Preview inventory states after batch
  const previewBatchInventories = () => {
    let srcPreview = [...currentSrcSlots];
    let dstPreview = [...currentDstSlots];

    for (const t of pendingTransfers) {
      // Update source
      srcPreview = srcPreview
        .map(s => s.item_id === t.item_id ? { ...s, quantity: s.quantity - t.amount } : s)
        .filter(s => s.quantity > 0);

      // Update destination
      const dstIdx = dstPreview.findIndex(s => s.item_id === t.item_id);
      if (dstIdx >= 0) {
        dstPreview = dstPreview.map(s => s.item_id === t.item_id ? { ...s, quantity: s.quantity + t.amount } : s);
      } else {
        dstPreview = [...dstPreview, { item_id: t.item_id, quantity: t.amount }];
      }
    }

    return { srcPreview, dstPreview };
  };

  const executeBatchTransfers = async () => {
    if (!currentSrcBlinding || !currentDstBlinding || !srcOnChain || !dstOnChain ||
        !effectiveAddress || pendingTransfers.length === 0) {
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
      // Fetch fresh inventory states
      const [fetchedSrc, fetchedDst] = await Promise.all([
        fetchFreshInventory(srcOnChain.id, useLocalSigner),
        fetchFreshInventory(dstOnChain.id, useLocalSigner),
      ]);

      const freshSrcOnChain = fetchedSrc || srcOnChain;
      const freshDstOnChain = fetchedDst || dstOnChain;

      const registryRoot = getRegistryRoot();
      const srcMaxCapacity = srcOnChain.maxCapacity;
      const dstMaxCapacity = dstOnChain.maxCapacity;

      // Pre-compute intermediate states and generate proofs sequentially
      // (transfers are dependent - each needs previous state)
      let srcSlots = [...currentSrcSlots];
      let dstSlots = [...currentDstSlots];
      let srcBlinding = currentSrcBlinding;
      let dstBlinding = currentDstBlinding;
      let srcNonce = freshSrcOnChain.nonce;
      let dstNonce = freshDstOnChain.nonce;

      const proofStart = performance.now();
      const transfers: TransferResult[] = [];

      for (const t of pendingTransfers) {
        const [srcNewBlinding, dstNewBlinding] = await Promise.all([
          api.generateBlinding(),
          api.generateBlinding(),
        ]);

        const srcVolume = calculateUsedVolume(srcSlots);
        const dstVolume = calculateUsedVolume(dstSlots);
        const itemVolume = ITEM_VOLUMES[t.item_id] ?? 0;

        const result = await api.proveTransfer(
          srcSlots, srcVolume, srcBlinding, srcNewBlinding, srcNonce, srcOnChain.id,
          dstSlots, dstVolume, dstBlinding, dstNewBlinding, dstNonce, dstOnChain.id,
          t.item_id, t.amount, itemVolume, registryRoot, srcMaxCapacity, dstMaxCapacity
        );

        transfers.push(result);

        // Update states for next iteration
        srcSlots = srcSlots
          .map(s => s.item_id === t.item_id ? { ...s, quantity: s.quantity - t.amount } : s)
          .filter(s => s.quantity > 0);

        const dstIdx = dstSlots.findIndex(s => s.item_id === t.item_id);
        if (dstIdx >= 0) {
          dstSlots = dstSlots.map(s => s.item_id === t.item_id ? { ...s, quantity: s.quantity + t.amount } : s);
        } else {
          dstSlots = [...dstSlots, { item_id: t.item_id, quantity: t.amount }];
        }

        srcBlinding = srcNewBlinding;
        dstBlinding = dstNewBlinding;
        srcNonce++;
        dstNonce++;
      }

      const proofEnd = performance.now();
      setProofTimeMs(Math.round(proofEnd - proofStart));

      const batchRes: BatchTransferResult = {
        transfers,
        finalSrcSlots: srcSlots,
        finalDstSlots: dstSlots,
        finalSrcBlinding: srcBlinding,
        finalDstBlinding: dstBlinding,
        proofTimeMs: Math.round(proofEnd - proofStart),
      };
      setBatchResult(batchRes);

      // Build PTB with all transfers
      const txOperations: BatchTransferTxOperation[] = transfers.map((r, i) => ({
        srcProof: hexToBytes(r.srcProof.proof),
        srcSignalHash: hexToBytes(r.srcProof.public_inputs[0]),
        srcNonce: BigInt(r.srcNonce),
        srcInventoryId: hexToBytes(r.srcInventoryId),
        srcRegistryRoot: hexToBytes(r.srcRegistryRoot),
        srcNewCommitment: hexToBytes(r.srcNewCommitment),
        dstProof: hexToBytes(r.dstProof.proof),
        dstSignalHash: hexToBytes(r.dstProof.public_inputs[0]),
        dstNonce: BigInt(r.dstNonce),
        dstInventoryId: hexToBytes(r.dstInventoryId),
        dstRegistryRoot: hexToBytes(r.dstRegistryRoot),
        dstNewCommitment: hexToBytes(r.dstNewCommitment),
        itemId: pendingTransfers[i].item_id,
        amount: BigInt(pendingTransfers[i].amount),
      }));

      const tx = buildBatchTransfersTx(
        packageId, srcOnChain.id, dstOnChain.id, volumeRegistryId, verifyingKeysId, txOperations
      );

      // Execute transaction
      const txStart = performance.now();
      let txResult;

      if (useLocalSigner && localAddress) {
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
        gasUsed?: { computationCost: string; storageCost: string; storageRebate: string };
      } | undefined;

      if (effects?.gasUsed) {
        const computation = BigInt(effects.gasUsed.computationCost);
        const storage = BigInt(effects.gasUsed.storageCost);
        const rebate = BigInt(effects.gasUsed.storageRebate);
        setGasCostMist(computation + storage - rebate);
      }

      if (effects?.status?.status === 'success') {
        setTxDigest(txResult.digest);
        setTransferComplete(true);

        // Update local storage
        const stored = JSON.parse(localStorage.getItem('inventory-blindings') || '{}');
        stored[srcOnChain.id] = { blinding: srcBlinding, slots: srcSlots };
        stored[dstOnChain.id] = { blinding: dstBlinding, slots: dstSlots };
        localStorage.setItem('inventory-blindings', JSON.stringify(stored));

        setSrcLocalData({ blinding: srcBlinding, slots: srcSlots });
        setDstLocalData({ blinding: dstBlinding, slots: dstSlots });
        setPendingTransfers([]);
      } else {
        throw new Error('Transaction failed: ' + effects?.status?.error);
      }
    } catch (err) {
      console.error('Batch transfer error:', err);
      setError(err instanceof Error ? err.message : 'Failed to execute batch transfers');
    } finally {
      setLoading(false);
    }
  };

  const initialized = mode === 'demo'
    ? source.blinding && destination.blinding
    : srcLocalData?.blinding && dstLocalData?.blinding && srcOnChain && dstOnChain;

  return (
    <div className="col">
      <div className="mb-2">
        <h1>TRANSFER</h1>
        <p className="text-muted">
          Transfer items between two private inventories with ZK proofs.
        </p>
      </div>

      {/* Mode Toggle */}
      <div className="btn-group mb-2">
        <button
          onClick={() => {
            setMode('demo');
            setProofResult(null);
            setTransferComplete(false);
            setTxDigest(null);
            setPendingTransfers([]);
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
            setTransferComplete(false);
            setTxDigest(null);
            setPendingTransfers([]);
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
            setTransferComplete(false);
            setTxDigest(null);
            setBatchResult(null);
          }}
          className={`btn btn-secondary ${mode === 'batch' ? 'active' : ''}`}
        >
          [BATCH]
        </button>
      </div>

      {/* Two inventory panels */}
      <div className="grid grid-2">
        <div className="col">
          <div className="row-between mb-1">
            <span className="text-uppercase">SOURCE INVENTORY</span>
            <span className="badge">YOUR INVENTORY</span>
          </div>
          {mode === 'demo' ? (
            <InventoryCard
              title="Source"
              slots={source.slots}
              commitment={source.commitment}
              onSlotClick={(_, slot) => setItemId(slot.item_id)}
              selectedSlot={source.slots.findIndex((s) => s.item_id === itemId)}
            />
          ) : (
            <div className="card">
              <div className="card-header">
                <div className="card-header-left"></div>
                <span className="card-title">SELECT SOURCE</span>
                <div className="card-header-right"></div>
              </div>
              <div className="card-body">
                <OnChainInventorySelector
                  selectedInventory={srcOnChain}
                  onSelect={(inv, data) => {
                    setSrcOnChain(inv);
                    setSrcLocalData(data);
                    if (data?.slots.length) {
                      setItemId(data.slots[0].item_id);
                    }
                  }}
                  label="Source Inventory"
                />
              </div>
            </div>
          )}
        </div>

        <div className="col">
          <div className="row-between mb-1">
            <span className="text-uppercase">DESTINATION INVENTORY</span>
            <span className="badge">RECIPIENT</span>
          </div>
          {mode === 'demo' ? (
            <InventoryCard
              title="Destination"
              slots={destination.slots}
              commitment={destination.commitment}
            />
          ) : (
            <div className="card">
              <div className="card-header">
                <div className="card-header-left"></div>
                <span className="card-title">SELECT DESTINATION</span>
                <div className="card-header-right"></div>
              </div>
              <div className="card-body">
                <OnChainInventorySelector
                  selectedInventory={dstOnChain}
                  onSelect={(inv, data) => {
                    setDstOnChain(inv);
                    setDstLocalData(data);
                  }}
                  label="Destination Inventory"
                />
                {srcOnChain && dstOnChain && srcOnChain.id === dstOnChain.id && (
                  <p className="text-small text-warning mt-1">
                    [!!] Source and destination cannot be the same inventory.
                  </p>
                )}
              </div>
            </div>
          )}
        </div>
      </div>

      {/* Transfer controls */}
      <div className="card">
        <div className="card-header">
          <div className="card-header-left"></div>
          <span className="card-title">TRANSFER ITEMS</span>
          <div className="card-header-right"></div>
        </div>
        <div className="card-body">
          {mode === 'demo' && (
            <div className="row-between mb-2">
              <span className="text-muted text-small">DEMO MODE</span>
              <button onClick={resetDemo} className="btn btn-secondary btn-small">
                [RESET]
              </button>
            </div>
          )}

          {mode === 'demo' && !initialized ? (
            <div className="text-center">
              <p className="text-muted mb-2">
                Initialize both inventories with blinding factors and commitments.
              </p>
              <button onClick={initializeBlindings} className="btn btn-primary">
                [INITIALIZE INVENTORIES]
              </button>
            </div>
          ) : mode === 'onchain' && !initialized ? (
            <div className="text-center text-muted">
              Select both source and destination inventories to transfer.
            </div>
          ) : (
            <div className="col">
              <div className="grid grid-2">
                <div className="input-group">
                  <label className="input-label">Item to Transfer</label>
                  <select
                    value={itemId}
                    onChange={(e) => setItemId(Number(e.target.value))}
                    className="select"
                    disabled={currentSrcSlots.length === 0}
                  >
                    {currentSrcSlots.length === 0 ? (
                      <option>No items available</option>
                    ) : (
                      currentSrcSlots.map((slot) => (
                        <option key={slot.item_id} value={slot.item_id}>
                          {ITEM_NAMES[slot.item_id] || `Item #${slot.item_id}`} ({slot.quantity} available)
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
                    max={sourceItem?.quantity || 1}
                    className="input"
                  />
                  {hasDstCapacityLimit && (
                    <p className="text-small text-muted mt-1">
                      Volume: {ITEM_VOLUMES[itemId] ?? 0} x {amount} = {(ITEM_VOLUMES[itemId] ?? 0) * amount}
                    </p>
                  )}
                </div>
              </div>

              {mode === 'batch' ? (
                <button
                  onClick={addToBatch}
                  disabled={
                    !canTransfer ||
                    !canTransferWithCapacity ||
                    !srcOnChain ||
                    !dstOnChain ||
                    srcOnChain?.id === dstOnChain?.id
                  }
                  className="btn btn-primary"
                  style={{ width: '100%' }}
                >
                  [+ ADD TO BATCH]
                </button>
              ) : (
                <button
                  onClick={handleTransfer}
                  disabled={
                    loading ||
                    !canTransfer ||
                    !canTransferWithCapacity ||
                    (mode === 'onchain' && srcOnChain?.id === dstOnChain?.id)
                  }
                  className="btn btn-primary"
                  style={{ width: '100%' }}
                >
                  {loading ? 'PROCESSING...' : `[${mode === 'onchain' ? 'TRANSFER ON-CHAIN' : 'TRANSFER'} ->]`}
                </button>
              )}
            </div>
          )}

          {!canTransfer && initialized && sourceItem && (
            <div className="alert alert-error mt-2">
              [!!] Insufficient balance: only have {sourceItem.quantity}
            </div>
          )}

          {!canTransferWithCapacity && initialized && canTransfer && (
            <div className="alert alert-error mt-2">
              [!!] Transfer would exceed destination inventory capacity!
            </div>
          )}

          {mode === 'onchain' && dstOnChain && dstMaxCapacity > 0 && (
            <div className="mt-2">
              <div className="text-small text-muted mb-1">DESTINATION CAPACITY</div>
              <CapacityBar slots={currentDstSlots} maxCapacity={dstMaxCapacity} />
              <CapacityPreview
                currentSlots={currentDstSlots}
                maxCapacity={dstMaxCapacity}
                itemId={itemId}
                amount={amount}
                isDeposit={true}
              />
            </div>
          )}
        </div>
      </div>

      {/* Batch: Pending Transfers & Preview */}
      {mode === 'batch' && srcOnChain && dstOnChain && srcLocalData && dstLocalData && (
        <div className="grid grid-2">
          <div className="card">
            <div className="card-header">
              <div className="card-header-left"></div>
              <span className="card-title">PENDING TRANSFERS ({pendingTransfers.length})</span>
              <div className="card-header-right">
                {pendingTransfers.length > 0 && (
                  <button onClick={clearBatch} className="btn btn-secondary btn-small">
                    [CLEAR]
                  </button>
                )}
              </div>
            </div>
            <div className="card-body">
              {pendingTransfers.length === 0 ? (
                <div className="text-muted text-center">No transfers queued</div>
              ) : (
                <div className="col">
                  {pendingTransfers.map((t) => (
                    <div key={t.id} className="row-between" style={{ padding: '0.5rem', background: 'var(--bg-secondary)', marginBottom: '0.5rem' }}>
                      <span>
                        {t.amount} {ITEM_NAMES[t.item_id] || `#${t.item_id}`}
                      </span>
                      <button
                        onClick={() => removeFromBatch(t.id)}
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

          <div className="card">
            <div className="card-header">
              <div className="card-header-left"></div>
              <span className="card-title">PREVIEW</span>
              <div className="card-header-right"></div>
            </div>
            <div className="card-body">
              <div className="grid grid-2">
                <div>
                  <div className="text-small text-muted mb-1">SOURCE AFTER</div>
                  <div className="col text-small">
                    {previewBatchInventories().srcPreview.map(s => (
                      <div key={s.item_id}>{ITEM_NAMES[s.item_id] || `#${s.item_id}`}: {s.quantity}</div>
                    ))}
                    {previewBatchInventories().srcPreview.length === 0 && <span className="text-muted">Empty</span>}
                  </div>
                </div>
                <div>
                  <div className="text-small text-muted mb-1">DEST AFTER</div>
                  <div className="col text-small">
                    {previewBatchInventories().dstPreview.map(s => (
                      <div key={s.item_id}>{ITEM_NAMES[s.item_id] || `#${s.item_id}`}: {s.quantity}</div>
                    ))}
                    {previewBatchInventories().dstPreview.length === 0 && <span className="text-muted">Empty</span>}
                  </div>
                </div>
              </div>
              <button
                onClick={executeBatchTransfers}
                disabled={loading || pendingTransfers.length === 0}
                className="btn btn-primary mt-2"
                style={{ width: '100%' }}
              >
                {loading
                  ? 'PROCESSING...'
                  : `[EXECUTE ${pendingTransfers.length} TRANSFER${pendingTransfers.length !== 1 ? 'S' : ''}]`}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Batch Results */}
      {mode === 'batch' && batchResult && txDigest && (
        <div className="alert alert-success">
          <div className="row-between">
            <span>[OK] BATCH TRANSFER SUCCESSFUL</span>
            <span className="text-small">
              <span className="badge">{batchResult.proofTimeMs}ms proofs</span>
              {txTimeMs !== null && <span className="badge" style={{ marginLeft: '0.5ch' }}>{txTimeMs}ms tx</span>}
              {gasCostMist !== null && <span className="badge" style={{ marginLeft: '0.5ch' }}>{formatGasCost(gasCostMist)}</span>}
            </span>
          </div>
          <div className="text-small mt-1">
            {batchResult.transfers.length} transfers executed atomically on Sui blockchain.
          </div>
          <code className="text-break text-small">{txDigest}</code>
        </div>
      )}

      {/* Results */}
      {loading && (
        <ProofLoading
          message={mode === 'batch'
            ? `Executing ${pendingTransfers.length} transfers...`
            : 'Generating transfer proof...'}
        />
      )}
      {error && <ProofError error={error} onRetry={mode === 'batch' ? executeBatchTransfers : handleTransfer} />}

      {mode !== 'batch' && proofResult && (
        <div className="col">
          {txDigest && (
            <div className="alert alert-success">
              <div className="row-between">
                <span>[OK] ON-CHAIN TRANSFER SUCCESSFUL</span>
                {(proofTimeMs !== null || txTimeMs !== null || gasCostMist !== null) && (
                  <span className="text-small">
                    {proofTimeMs !== null && <span className="badge">{proofTimeMs}ms proof</span>}
                    {txTimeMs !== null && <span className="badge" style={{ marginLeft: '0.5ch' }}>{txTimeMs}ms tx</span>}
                    {gasCostMist !== null && <span className="badge" style={{ marginLeft: '0.5ch' }}>{formatGasCost(gasCostMist)}</span>}
                  </span>
                )}
              </div>
              <div className="text-small mt-1">Transfer executed on Sui blockchain with ZK proof verification.</div>
              <code className="text-break text-small">{txDigest}</code>
            </div>
          )}

          {transferComplete && !txDigest && (
            <div className="alert alert-success">
              <div className="row-between">
                <span>[OK] TRANSFER COMPLETE!</span>
                {proofTimeMs !== null && <span className="badge">{proofTimeMs}ms</span>}
              </div>
              <div className="text-small">
                {amount} {ITEM_NAMES[itemId] || `Item #${itemId}`} transferred from source to destination.
              </div>
            </div>
          )}

          <div className="grid grid-2">
            <div className="card-simple">
              <div className="text-small text-muted mb-1">SRC NEW COMMITMENT</div>
              <code className="text-break text-small">{proofResult.srcNewCommitment}</code>
            </div>
            <div className="card-simple">
              <div className="text-small text-muted mb-1">DST NEW COMMITMENT</div>
              <code className="text-break text-small">{proofResult.dstNewCommitment}</code>
            </div>
          </div>

          <div className="text-small text-success mb-2">
            [OK] Proved valid transfer of <strong>{amount}</strong>{' '}
            <strong>{ITEM_NAMES[itemId] || `Item #${itemId}`}</strong> between inventories.
          </div>

          <div className="grid grid-2">
            <ProofResult
              result={proofResult.srcProof}
              title="Source Withdrawal Proof"
            />
            <ProofResult
              result={proofResult.dstProof}
              title="Destination Deposit Proof"
            />
          </div>
        </div>
      )}

      {!loading && !proofResult && !error && initialized && (
        <div className="card">
          <div className="card-header">
            <div className="card-header-left"></div>
            <span className="card-title">WHAT GETS PROVEN</span>
            <div className="card-header-right"></div>
          </div>
          <div className="card-body">
            <div className="grid grid-2">
              <div>
                <div className="text-small text-muted mb-1">SOURCE</div>
                <div className="col text-small">
                  <div>[OK] Old commitment is valid</div>
                  <div>[OK] Has sufficient balance</div>
                  <div>[OK] New commitment = old - amount</div>
                </div>
              </div>
              <div>
                <div className="text-small text-muted mb-1">DESTINATION</div>
                <div className="col text-small">
                  <div>[OK] Old commitment is valid</div>
                  <div>[OK] New commitment = old + amount</div>
                  <div>[OK] Same item_id and amount</div>
                </div>
              </div>
            </div>
            {mode === 'onchain' && (
              <div className="mt-2 text-small text-muted" style={{ borderTop: '1px solid var(--border)', paddingTop: '0.5rem' }}>
                Both inventories' commitments will be updated on-chain after ZK proof verification.
                {hasDstCapacityLimit && (
                  <span className="text-accent"> Capacity-aware proof verifies destination doesn't exceed volume limit.</span>
                )}
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
