//! HTTP request handlers for SMT-based proof generation.

use std::sync::Arc;

use ark_bn254::Fr;
use ark_ff::PrimeField;
use ark_serialize::CanonicalSerialize;
use ark_std::rand::Rng;
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use inventory_circuits::{
    poseidon_config,
    signal::OpType,
    smt::{SparseMerkleTree, DEFAULT_DEPTH},
    smt_commitment::create_smt_commitment,
};
use inventory_prover::{prove, InventoryState};

use crate::AppState;

/// Health check response
#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
}

pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

/// Item in inventory for API requests
#[derive(Debug, Deserialize)]
pub struct ItemRequest {
    pub item_id: u64,
    pub quantity: u64,
}

/// Create an InventoryState from API request items
fn parse_inventory_state(items: &[ItemRequest], volume: u64, blinding: Fr) -> InventoryState {
    let config = Arc::new(poseidon_config::<Fr>());
    let pairs: Vec<(u64, u64)> = items.iter().map(|i| (i.item_id, i.quantity)).collect();
    let tree = SparseMerkleTree::from_items(&pairs, DEFAULT_DEPTH, config);

    InventoryState {
        tree,
        current_volume: volume,
        blinding,
    }
}

/// Parse hex string to Fr (little-endian, for blinding factors etc)
fn parse_fr(hex: &str) -> Result<Fr, String> {
    let bytes = hex::decode(hex.trim_start_matches("0x"))
        .map_err(|e| format!("Invalid hex: {}", e))?;

    if bytes.len() != 32 {
        return Err("Field element must be 32 bytes".to_string());
    }

    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);

    Ok(Fr::from_le_bytes_mod_order(&arr))
}

/// Parse hex string to Fr (big-endian, for Sui object IDs)
/// Sui object IDs are big-endian, so we reverse bytes before interpreting as LE field element
fn parse_fr_be(hex: &str) -> Result<Fr, String> {
    let bytes = hex::decode(hex.trim_start_matches("0x"))
        .map_err(|e| format!("Invalid hex: {}", e))?;

    if bytes.len() != 32 {
        return Err("Field element must be 32 bytes".to_string());
    }

    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    arr.reverse(); // Convert BE to LE

    Ok(Fr::from_le_bytes_mod_order(&arr))
}

/// Serialize Fr to hex string (little-endian)
fn serialize_fr(f: &Fr) -> String {
    let mut bytes = Vec::new();
    f.serialize_compressed(&mut bytes).unwrap();
    format!("0x{}", hex::encode(bytes))
}

/// Serialize Fr to hex string (big-endian, for Sui object IDs)
fn serialize_fr_be(f: &Fr) -> String {
    let mut bytes = Vec::new();
    f.serialize_compressed(&mut bytes).unwrap();
    bytes.reverse(); // Convert LE to BE
    format!("0x{}", hex::encode(bytes))
}

/// Common proof response
#[derive(Serialize)]
pub struct ProofResponse {
    pub proof: String,
    pub public_inputs: Vec<String>,
}

/// Error response
#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

// ============ State Transition (Deposit/Withdraw) ============

#[derive(Deserialize)]
pub struct StateTransitionRequest {
    /// Current inventory items
    pub inventory: Vec<ItemRequest>,
    /// Current total volume
    pub current_volume: u64,
    /// Current blinding factor
    pub old_blinding: String,
    /// New blinding factor
    pub new_blinding: String,
    /// Item ID to deposit/withdraw
    pub item_id: u64,
    /// Amount to deposit/withdraw
    pub amount: u64,
    /// Volume per unit of this item
    pub item_volume: u64,
    /// Registry root (for volume lookup verification)
    pub registry_root: String,
    /// Maximum allowed capacity
    pub max_capacity: u64,
    /// Current nonce from on-chain inventory (for replay protection)
    pub nonce: u64,
    /// Inventory object ID as hex string (for cross-inventory protection)
    pub inventory_id: String,
    /// Operation type: "deposit" or "withdraw"
    pub op_type: String,
}

#[derive(Serialize)]
pub struct StateTransitionResponse {
    pub proof: String,
    pub public_inputs: Vec<String>,
    pub new_commitment: String,
    pub new_volume: u64,
    /// Nonce used in this proof (for on-chain verification)
    pub nonce: u64,
    /// Inventory ID used in this proof (for on-chain verification)
    pub inventory_id: String,
    /// Registry root used in this proof (for on-chain verification)
    pub registry_root: String,
}

pub async fn prove_state_transition(
    State(state): State<Arc<RwLock<AppState>>>,
    Json(req): Json<StateTransitionRequest>,
) -> impl IntoResponse {
    let old_blinding = match parse_fr(&req.old_blinding) {
        Ok(b) => b,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })).into_response(),
    };

    let new_blinding = match parse_fr(&req.new_blinding) {
        Ok(b) => b,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })).into_response(),
    };

    let registry_root = match parse_fr(&req.registry_root) {
        Ok(r) => r,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })).into_response(),
    };

    // Parse inventory_id - interpreted as LE field element (with modular reduction if needed)
    let inventory_id = match parse_fr(&req.inventory_id) {
        Ok(id) => id,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })).into_response(),
    };

    let op_type = match req.op_type.to_lowercase().as_str() {
        "deposit" => OpType::Deposit,
        "withdraw" => OpType::Withdraw,
        _ => return (StatusCode::BAD_REQUEST, Json(ErrorResponse {
            error: "op_type must be 'deposit' or 'withdraw'".to_string()
        })).into_response(),
    };

    let inventory_state = parse_inventory_state(&req.inventory, req.current_volume, old_blinding);

    let app_state = state.read().await;

    match prove::prove_state_transition(
        &app_state.keys.state_transition.proving_key,
        &inventory_state,
        new_blinding,
        req.item_id,
        req.amount,
        req.item_volume,
        registry_root,
        req.max_capacity,
        req.nonce,
        inventory_id,
        op_type,
    ) {
        Ok(result) => {
            let proof_bytes = result.proof.serialize_proof().unwrap();
            let response = StateTransitionResponse {
                proof: format!("0x{}", hex::encode(proof_bytes)),
                public_inputs: result.proof
                    .public_inputs
                    .iter()
                    .map(serialize_fr)
                    .collect(),
                new_commitment: serialize_fr(&result.new_commitment),
                new_volume: result.new_state.current_volume,
                nonce: result.nonce,
                // Return serialized field element bytes - this matches what the circuit used
                // after modular reduction (for object IDs exceeding BN254 field order)
                inventory_id: serialize_fr(&inventory_id),
                registry_root: serialize_fr(&result.registry_root),
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

// ============ Item Exists ============

#[derive(Deserialize)]
pub struct ItemExistsRequest {
    /// Current inventory items
    pub inventory: Vec<ItemRequest>,
    /// Current total volume
    pub current_volume: u64,
    /// Blinding factor
    pub blinding: String,
    /// Item ID to prove
    pub item_id: u64,
    /// Minimum quantity to prove
    pub min_quantity: u64,
}

pub async fn prove_item_exists(
    State(state): State<Arc<RwLock<AppState>>>,
    Json(req): Json<ItemExistsRequest>,
) -> impl IntoResponse {
    let blinding = match parse_fr(&req.blinding) {
        Ok(b) => b,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })).into_response(),
    };

    let inventory_state = parse_inventory_state(&req.inventory, req.current_volume, blinding);

    let app_state = state.read().await;

    match prove::prove_item_exists(
        &app_state.keys.item_exists.proving_key,
        &inventory_state,
        req.item_id,
        req.min_quantity,
    ) {
        Ok(proof_with_inputs) => {
            let proof_bytes = proof_with_inputs.serialize_proof().unwrap();
            let response = ProofResponse {
                proof: format!("0x{}", hex::encode(proof_bytes)),
                public_inputs: proof_with_inputs
                    .public_inputs
                    .iter()
                    .map(serialize_fr)
                    .collect(),
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

// ============ Capacity ============

#[derive(Deserialize)]
pub struct CapacityRequest {
    /// Current inventory items
    pub inventory: Vec<ItemRequest>,
    /// Current total volume
    pub current_volume: u64,
    /// Blinding factor
    pub blinding: String,
    /// Maximum allowed capacity
    pub max_capacity: u64,
}

pub async fn prove_capacity(
    State(state): State<Arc<RwLock<AppState>>>,
    Json(req): Json<CapacityRequest>,
) -> impl IntoResponse {
    let blinding = match parse_fr(&req.blinding) {
        Ok(b) => b,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })).into_response(),
    };

    let inventory_state = parse_inventory_state(&req.inventory, req.current_volume, blinding);

    let app_state = state.read().await;

    match prove::prove_capacity(
        &app_state.keys.capacity.proving_key,
        &inventory_state,
        req.max_capacity,
    ) {
        Ok(proof_with_inputs) => {
            let proof_bytes = proof_with_inputs.serialize_proof().unwrap();
            let response = ProofResponse {
                proof: format!("0x{}", hex::encode(proof_bytes)),
                public_inputs: proof_with_inputs
                    .public_inputs
                    .iter()
                    .map(serialize_fr)
                    .collect(),
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

// ============ Utilities ============

#[derive(Deserialize)]
pub struct CreateCommitmentRequest {
    /// Inventory items
    pub inventory: Vec<ItemRequest>,
    /// Current total volume
    pub current_volume: u64,
    /// Blinding factor
    pub blinding: String,
}

#[derive(Serialize)]
pub struct CreateCommitmentResponse {
    pub commitment: String,
    pub inventory_root: String,
}

pub async fn create_commitment(
    Json(req): Json<CreateCommitmentRequest>,
) -> impl IntoResponse {
    let blinding = match parse_fr(&req.blinding) {
        Ok(b) => b,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })).into_response(),
    };

    let config = Arc::new(poseidon_config::<Fr>());
    let pairs: Vec<(u64, u64)> = req.inventory.iter().map(|i| (i.item_id, i.quantity)).collect();
    let tree = SparseMerkleTree::from_items(&pairs, DEFAULT_DEPTH, config.clone());

    let inventory_root = tree.root();
    let commitment = create_smt_commitment(
        inventory_root,
        req.current_volume,
        blinding,
        &config,
    );

    (
        StatusCode::OK,
        Json(CreateCommitmentResponse {
            commitment: serialize_fr(&commitment),
            inventory_root: serialize_fr(&inventory_root),
        }),
    )
        .into_response()
}

#[derive(Serialize)]
pub struct GenerateBlindingResponse {
    pub blinding: String,
}

pub async fn generate_blinding() -> Json<GenerateBlindingResponse> {
    let mut rng = ark_std::rand::thread_rng();
    let blinding: Fr = rng.gen();

    Json(GenerateBlindingResponse {
        blinding: serialize_fr(&blinding),
    })
}
