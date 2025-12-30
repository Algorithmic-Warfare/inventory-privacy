import { Link } from 'react-router-dom';

export function Home() {
  return (
    <div className="col">
      {/* Hero */}
      <div className="text-center my-2">
        <h1 className="mb-1">INVENTORY PRIVACY POC</h1>
        <p className="text-muted">
          Hidden on-chain inventory state with verifiable zero-knowledge operations.
          Prove what you have without revealing how much.
        </p>
      </div>

      {/* How it works */}
      <div className="card">
        <div className="card-header">
          <div className="card-header-left"></div>
          <span className="card-title">HOW IT WORKS</span>
          <div className="card-header-right"></div>
        </div>
        <div className="card-body">
          <div className="grid grid-3">
            <div className="card-simple text-center">
              <div className="text-accent mb-1">[1]</div>
              <div className="mb-1">SPARSE MERKLE TREE</div>
              <div className="text-muted text-small">
                Items stored in a depth-12 SMT supporting up to 4,096 item types with O(log n) proofs.
              </div>
            </div>

            <div className="card-simple text-center">
              <div className="text-accent mb-1">[2]</div>
              <div className="mb-1">GROTH16 PROOFS</div>
              <div className="text-muted text-small">
                Fast ZK proofs (~100ms) verified on-chain using Sui's native Groth16 verifier.
              </div>
            </div>

            <div className="card-simple text-center">
              <div className="text-accent mb-1">[3]</div>
              <div className="mb-1">SIGNAL HASH</div>
              <div className="text-muted text-small">
                Single public input pattern keeps on-chain verification gas-efficient.
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Architecture */}
      <div className="card">
        <div className="card-header">
          <div className="card-header-left"></div>
          <span className="card-title">SMT-BASED COMMITMENT</span>
          <div className="card-header-right"></div>
        </div>
        <div className="card-body">
          <pre style={{ background: 'var(--bg-secondary)', padding: '1rem', overflow: 'auto' }}>
{`// Inventory stored as Sparse Merkle Tree (depth 12)
inventory_root = SMT.root()  // Poseidon hash of item quantities

// Commitment binds SMT root + volume + blinding
commitment = Poseidon(inventory_root, current_volume, blinding)

ON-CHAIN:  commitment, nonce, max_capacity
OFF-CHAIN: SMT state, blinding factor (kept secret)`}
          </pre>
          <div className="grid grid-3 mt-2">
            <div className="text-center">
              <div className="text-accent">4,096</div>
              <div className="text-small text-muted">Max Item Types</div>
            </div>
            <div className="text-center">
              <div className="text-accent">~100ms</div>
              <div className="text-small text-muted">Proof Generation</div>
            </div>
            <div className="text-center">
              <div className="text-accent">3</div>
              <div className="text-small text-muted">Circuit Types</div>
            </div>
          </div>
        </div>
      </div>

      {/* Available proofs */}
      <div className="card">
        <div className="card-header">
          <div className="card-header-left"></div>
          <span className="card-title">CIRCUIT TYPES</span>
          <div className="card-header-right"></div>
        </div>
        <div className="card-body">
          <div className="grid grid-3">
            <div className="card-simple">
              <div className="text-accent mb-1">STATE TRANSITION</div>
              <div className="text-small text-muted mb-2">
                Unified circuit for deposit/withdraw operations with capacity enforcement.
              </div>
              <div className="text-small">
                <div>[OK] Verify old commitment</div>
                <div>[OK] Check balance sufficient</div>
                <div>[OK] Enforce capacity limit</div>
                <div>[OK] Compute new commitment</div>
              </div>
              <div className="badge mt-2">8,255 constraints</div>
            </div>

            <div className="card-simple">
              <div className="text-accent mb-1">ITEM EXISTS</div>
              <div className="text-small text-muted mb-2">
                Prove ownership of items without revealing exact quantities.
              </div>
              <div className="text-small">
                <div>[OK] Verify commitment</div>
                <div>[OK] Prove SMT membership</div>
                <div>[OK] quantity &gt;= min_qty</div>
              </div>
              <div className="badge mt-2">4,124 constraints</div>
            </div>

            <div className="card-simple">
              <div className="text-accent mb-1">CAPACITY PROOF</div>
              <div className="text-small text-muted mb-2">
                Prove volume compliance without revealing contents.
              </div>
              <div className="text-small">
                <div>[OK] Verify commitment</div>
                <div>[OK] volume &lt;= max_capacity</div>
              </div>
              <div className="badge mt-2">724 constraints</div>
            </div>
          </div>
        </div>
      </div>

      {/* Operations */}
      <div className="card">
        <div className="card-header">
          <div className="card-header-left"></div>
          <span className="card-title">SUPPORTED OPERATIONS</span>
          <div className="card-header-right"></div>
        </div>
        <div className="card-body">
          <div className="grid grid-2">
            <div className="card-simple">
              <div className="row-between mb-1">
                <span className="text-accent">DEPOSIT</span>
                <span className="badge">StateTransition</span>
              </div>
              <div className="text-small text-muted">
                Add items to inventory with capacity check
              </div>
            </div>

            <div className="card-simple">
              <div className="row-between mb-1">
                <span className="text-accent">WITHDRAW</span>
                <span className="badge">StateTransition</span>
              </div>
              <div className="text-small text-muted">
                Remove items with balance verification
              </div>
            </div>

            <div className="card-simple">
              <div className="row-between mb-1">
                <span className="text-accent">TRANSFER</span>
                <span className="badge">2x StateTransition</span>
              </div>
              <div className="text-small text-muted">
                Atomic move between inventories (src withdraw + dst deposit)
              </div>
            </div>

            <div className="card-simple">
              <div className="row-between mb-1">
                <span className="text-accent">PROVE OWNERSHIP</span>
                <span className="badge">ItemExists</span>
              </div>
              <div className="text-small text-muted">
                Prove "I have &gt;= N of item X" without revealing actual qty
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* CTA */}
      <div className="row" style={{ justifyContent: 'center' }}>
        <Link to="/inventory" className="btn btn-primary">
          [CREATE INVENTORY]
        </Link>
        <Link to="/prove" className="btn btn-secondary">
          [TRY PROOF DEMO]
        </Link>
        <Link to="/explorer" className="btn btn-secondary">
          [VIEW EXPLORER]
        </Link>
      </div>
    </div>
  );
}
