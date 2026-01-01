import { useState } from 'react';
import {
  useCurrentAccount,
  useSuiClient,
} from '@mysten/dapp-kit';
import { ProofLoading, ProofError } from '../components/ProofResult';
import {
  OnChainInventorySelector,
  type LocalInventoryData,
} from '../components/OnChainInventorySelector';
import { useContractAddresses } from '../sui/ContractConfig';
import { buildVerifyItemExistsTx, hexToBytes } from '../sui/transactions';
import { ITEM_NAMES, calculateUsedVolume } from '../types';
import * as api from '../api/client';
import type { ProofResult as ProofResultType } from '../types';
import type { OnChainInventory } from '../sui/hooks';
import { hasLocalSigner, getLocalAddress } from '../sui/localSigner';

/** Pending proof request in queue */
interface PendingProof {
  id: string;
  item_id: number;
  min_quantity: number;
}

/** Result of a single proof */
interface ProofResultItem {
  item_id: number;
  min_quantity: number;
  proof: ProofResultType;
  verified?: boolean;
  verifyError?: string;
}

export function ProveOwnership() {
  const account = useCurrentAccount();
  const client = useSuiClient();
  const { packageId, verifyingKeysId } = useContractAddresses();

  // Use local signer if available, otherwise fall back to wallet
  const useLocalSignerFlag = hasLocalSigner();
  const localAddress = useLocalSignerFlag ? getLocalAddress() : null;
  const effectiveAddress = localAddress || account?.address;

  // Inventory selection
  const [selectedInventory, setSelectedInventory] = useState<OnChainInventory | null>(null);
  const [localData, setLocalData] = useState<LocalInventoryData | null>(null);

  // Proof parameters
  const [selectedItemId, setSelectedItemId] = useState(1);
  const [minQuantity, setMinQuantity] = useState(50);

  // Queue state
  const [pendingProofs, setPendingProofs] = useState<PendingProof[]>([]);
  const [results, setResults] = useState<ProofResultItem[] | null>(null);

  // Execution state
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [proofTimeMs, setProofTimeMs] = useState<number | null>(null);
  const [verifyTimeMs, setVerifyTimeMs] = useState<number | null>(null);

  const currentSlots = localData?.slots || [];
  const currentBlinding = localData?.blinding;
  const selectedItem = currentSlots.find((s) => s.item_id === selectedItemId);
  const canAdd = selectedItem && selectedItem.quantity >= minQuantity && currentBlinding;

  const handleInventorySelect = (
    inv: OnChainInventory | null,
    data: LocalInventoryData | null
  ) => {
    setSelectedInventory(inv);
    setLocalData(data);
    setPendingProofs([]);
    setResults(null);
    setError(null);
    if (data?.slots.length) {
      setSelectedItemId(data.slots[0].item_id);
    }
  };

  // Queue handlers
  const addToQueue = () => {
    const proof: PendingProof = {
      id: crypto.randomUUID(),
      item_id: selectedItemId,
      min_quantity: minQuantity,
    };
    setPendingProofs([...pendingProofs, proof]);
  };

  const removeFromQueue = (id: string) => {
    setPendingProofs(pendingProofs.filter(p => p.id !== id));
  };

  const clearQueue = () => {
    setPendingProofs([]);
    setResults(null);
    setError(null);
  };

  // Execute all proofs in parallel
  const execute = async () => {
    if (!currentBlinding || pendingProofs.length === 0 || !selectedInventory || !effectiveAddress) return;

    setLoading(true);
    setError(null);
    setResults(null);
    setProofTimeMs(null);
    setVerifyTimeMs(null);

    try {
      const currentVolume = calculateUsedVolume(currentSlots);
      const proofStart = performance.now();

      // Generate all proofs in parallel
      const proofPromises = pendingProofs.map(p =>
        api.proveItemExists(
          currentSlots,
          currentVolume,
          currentBlinding!,
          p.item_id,
          p.min_quantity
        ).then(proof => ({
          item_id: p.item_id,
          min_quantity: p.min_quantity,
          proof,
        }))
      );

      const proofResults = await Promise.all(proofPromises);
      const proofEnd = performance.now();
      setProofTimeMs(Math.round(proofEnd - proofStart));

      // Verify all proofs on-chain in parallel
      const verifyStart = performance.now();
      const verifyPromises = proofResults.map(async (r) => {
        try {
          const proofBytes = hexToBytes(r.proof.proof);
          const signalHashBytes = hexToBytes(r.proof.public_inputs[0]);
          const tx = buildVerifyItemExistsTx(
            packageId,
            selectedInventory.id,
            verifyingKeysId,
            proofBytes,
            signalHashBytes
          );

          const devInspectResult = await client.devInspectTransactionBlock({
            transactionBlock: tx as unknown as Parameters<typeof client.devInspectTransactionBlock>[0]['transactionBlock'],
            sender: effectiveAddress,
          });

          const returnValues = devInspectResult.results?.[0]?.returnValues;
          if (returnValues && returnValues.length > 0) {
            const boolResult = returnValues[returnValues.length - 1];
            const verified = boolResult[0][0] === 1;
            return { ...r, verified };
          }
          return { ...r, verified: false, verifyError: 'No return value' };
        } catch (err) {
          return { ...r, verified: false, verifyError: err instanceof Error ? err.message : 'Unknown error' };
        }
      });

      const verifiedResults = await Promise.all(verifyPromises);
      const verifyEnd = performance.now();
      setVerifyTimeMs(Math.round(verifyEnd - verifyStart));

      setResults(verifiedResults);
      setPendingProofs([]);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to generate proofs');
    } finally {
      setLoading(false);
    }
  };

  const allVerified = results?.every(r => r.verified) ?? false;
  const anyFailed = results?.some(r => !r.verified) ?? false;

  return (
    <div className="col">
      <div className="mb-2">
        <h1>PROVE OWNERSHIP</h1>
        <p className="text-muted">
          Prove you have at least N items without revealing your actual quantity.
        </p>
      </div>

      <div className="grid grid-2">
        {/* Left: Configuration */}
        <div className="col">
          <div className="card">
            <div className="card-header">
              <div className="card-header-left"></div>
              <span className="card-title">SELECT INVENTORY</span>
              <div className="card-header-right"></div>
            </div>
            <div className="card-body">
              <OnChainInventorySelector
                selectedInventory={selectedInventory}
                onSelect={handleInventorySelect}
              />
            </div>
          </div>

          {selectedInventory && localData && (
            <>
              <div className="card">
                <div className="card-header">
                  <div className="card-header-left"></div>
                  <span className="card-title">PROOF PARAMETERS</span>
                  <div className="card-header-right"></div>
                </div>
                <div className="card-body">
                  <div className="input-group">
                    <label className="input-label">Item to Prove</label>
                    <select
                      value={selectedItemId}
                      onChange={(e) => setSelectedItemId(Number(e.target.value))}
                      className="select"
                      disabled={currentSlots.length === 0}
                    >
                      {currentSlots.length === 0 ? (
                        <option>No items available</option>
                      ) : (
                        currentSlots.map((slot) => (
                          <option key={slot.item_id} value={slot.item_id}>
                            {ITEM_NAMES[slot.item_id] || `Item #${slot.item_id}`} (you have {slot.quantity})
                          </option>
                        ))
                      )}
                    </select>
                  </div>

                  <div className="input-group">
                    <label className="input-label">Minimum Quantity to Prove</label>
                    <input
                      type="number"
                      value={minQuantity}
                      onChange={(e) => setMinQuantity(Number(e.target.value))}
                      min={1}
                      className="input"
                    />
                    {selectedItem && (
                      <p className={`text-small mt-1 ${selectedItem.quantity >= minQuantity ? 'text-success' : 'text-error'}`}>
                        {selectedItem.quantity >= minQuantity
                          ? `[OK] You have ${selectedItem.quantity}, proof will succeed`
                          : `[!!] You only have ${selectedItem.quantity}, proof will fail`}
                      </p>
                    )}
                  </div>

                  <button
                    onClick={addToQueue}
                    disabled={!canAdd}
                    className="btn btn-primary"
                    style={{ width: '100%' }}
                  >
                    [+ ADD TO QUEUE]
                  </button>
                </div>
              </div>

              <div className="card">
                <div className="card-header">
                  <div className="card-header-left"></div>
                  <span className="card-title">PROOF QUEUE ({pendingProofs.length})</span>
                  <div className="card-header-right">
                    {pendingProofs.length > 0 && (
                      <button onClick={clearQueue} className="btn btn-secondary btn-small">
                        [CLEAR]
                      </button>
                    )}
                  </div>
                </div>
                <div className="card-body">
                  {pendingProofs.length === 0 ? (
                    <div className="text-muted text-center">No proofs queued</div>
                  ) : (
                    <div className="col">
                      {pendingProofs.map((p) => (
                        <div key={p.id} className="row-between" style={{ padding: '0.5rem', background: 'var(--bg-secondary)', marginBottom: '0.5rem' }}>
                          <span>
                            {ITEM_NAMES[p.item_id] || `#${p.item_id}`} &gt;= {p.min_quantity}
                          </span>
                          <button
                            onClick={() => removeFromQueue(p.id)}
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
            </>
          )}
        </div>

        {/* Right: Results & Execution */}
        <div className="col">
          {selectedInventory && localData && (
            <div className="card">
              <div className="card-header">
                <div className="card-header-left"></div>
                <span className="card-title">EXECUTE</span>
                <div className="card-header-right"></div>
              </div>
              <div className="card-body">
                <div className="text-small text-muted mb-2">
                  {pendingProofs.length === 0
                    ? 'Add proofs to the queue to execute them in parallel.'
                    : `${pendingProofs.length} proof${pendingProofs.length !== 1 ? 's' : ''} queued for parallel generation and on-chain verification.`}
                </div>
                <button
                  onClick={execute}
                  disabled={loading || pendingProofs.length === 0 || !currentBlinding || !effectiveAddress}
                  className="btn btn-primary"
                  style={{ width: '100%' }}
                >
                  {loading
                    ? 'PROCESSING...'
                    : `[PROVE & VERIFY ${pendingProofs.length} ITEM${pendingProofs.length !== 1 ? 'S' : ''}]`}
                </button>
                {!effectiveAddress && (
                  <p className="text-small text-error mt-1">[!!] No signer available</p>
                )}
              </div>
            </div>
          )}

          {loading && (
            <ProofLoading
              message={`Generating ${pendingProofs.length} proof${pendingProofs.length !== 1 ? 's' : ''} and verifying on-chain...`}
            />
          )}

          {error && <ProofError error={error} onRetry={execute} />}

          {/* Results */}
          {results && (
            <>
              <div className={`alert ${allVerified ? 'alert-success' : anyFailed ? 'alert-error' : 'alert-warning'}`}>
                <div className="row-between">
                  <span>
                    {allVerified
                      ? `[OK] ALL ${results.length} PROOF${results.length !== 1 ? 'S' : ''} VERIFIED`
                      : anyFailed
                      ? `[!!] SOME PROOFS FAILED VERIFICATION`
                      : `[?] MIXED RESULTS`}
                  </span>
                  <span className="text-small">
                    {proofTimeMs !== null && <span className="badge">{proofTimeMs}ms proof</span>}
                    {verifyTimeMs !== null && <span className="badge" style={{ marginLeft: '0.5ch' }}>{verifyTimeMs}ms verify</span>}
                  </span>
                </div>
              </div>

              <div className="card">
                <div className="card-header">
                  <div className="card-header-left"></div>
                  <span className="card-title">PROOF RESULTS</span>
                  <div className="card-header-right"></div>
                </div>
                <div className="card-body">
                  <div className="col">
                    {results.map((r, i) => (
                      <div key={i} className="card-simple mb-1">
                        <div className="row-between">
                          <span className={`text-small ${r.verified ? 'text-success' : 'text-error'}`}>
                            {r.verified ? '[OK]' : '[!!]'} {ITEM_NAMES[r.item_id] || `#${r.item_id}`} &gt;= {r.min_quantity}
                          </span>
                          <span className="text-small text-muted">
                            {r.verified ? 'verified on-chain' : r.verifyError || 'verification failed'}
                          </span>
                        </div>
                        <div className="text-small text-muted mt-1">
                          <code>{r.proof.proof.slice(0, 40)}...</code>
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              </div>
            </>
          )}

          {/* What this proves info */}
          {selectedInventory && localData && !results && !loading && !error && (
            <>
              <div className="card-simple" style={{ background: 'var(--accent-subdued)' }}>
                <div className="text-accent mb-1">WHAT THIS PROVES</div>
                <p className="text-small">
                  "I have at least <strong>{minQuantity}</strong> of{' '}
                  <strong>{ITEM_NAMES[selectedItemId] || `Item #${selectedItemId}`}</strong>"
                </p>
                <div className="divider"></div>
                <div className="text-small text-muted">
                  <div>REVEALED: commitment, item_id, min_quantity</div>
                  <div>HIDDEN: actual qty ({selectedItem?.quantity}), other items, blinding</div>
                </div>
              </div>

              <div className="card">
                <div className="card-header">
                  <div className="card-header-left"></div>
                  <span className="card-title">HOW IT WORKS</span>
                  <div className="card-header-right"></div>
                </div>
                <div className="card-body">
                  <div className="col text-small">
                    <div>[1] Queue multiple item existence proofs</div>
                    <div>[2] All proofs generated IN PARALLEL</div>
                    <div>[3] Each proof verified on Sui blockchain via Groth16</div>
                    <div>[4] Verifier learns only that you meet the threshold</div>
                  </div>
                </div>
              </div>
            </>
          )}

          {/* Empty state before inventory selected */}
          {!selectedInventory && (
            <div className="card">
              <div className="card-header">
                <div className="card-header-left"></div>
                <span className="card-title">HOW IT WORKS</span>
                <div className="card-header-right"></div>
              </div>
              <div className="card-body">
                <div className="col text-small">
                  <div>[1] Select an on-chain inventory with local data</div>
                  <div>[2] Queue proofs for items you want to prove</div>
                  <div>[3] Execute to generate proofs in parallel</div>
                  <div>[4] Proofs are verified on-chain via Groth16</div>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
