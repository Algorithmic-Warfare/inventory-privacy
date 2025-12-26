//! HTTP request handlers for proof generation.

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
    commitment::{create_inventory_commitment, poseidon_config},
    Inventory,
};
use inventory_prover::prove;

use crate::AppState;

/// Health check response
#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
}

pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

/// Inventory slot in API requests
#[derive(Debug, Deserialize)]
pub struct SlotRequest {
    pub item_id: u32,
    pub quantity: u64,
}

/// Convert API inventory to internal format
fn parse_inventory(slots: &[SlotRequest]) -> Inventory {
    let items: Vec<(u32, u64)> = slots.iter().map(|s| (s.item_id, s.quantity)).collect();
    Inventory::from_items(&items)
}

/// Parse hex string to Fr
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

/// Serialize Fr to hex string
fn serialize_fr(f: &Fr) -> String {
    let mut bytes = Vec::new();
    f.serialize_compressed(&mut bytes).unwrap();
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

// ============ Item Exists ============

#[derive(Deserialize)]
pub struct ItemExistsRequest {
    pub inventory: Vec<SlotRequest>,
    pub blinding: String,
    pub item_id: u32,
    pub min_quantity: u64,
}

pub async fn prove_item_exists(
    State(state): State<Arc<RwLock<AppState>>>,
    Json(req): Json<ItemExistsRequest>,
) -> impl IntoResponse {
    let inventory = parse_inventory(&req.inventory);

    let blinding = match parse_fr(&req.blinding) {
        Ok(b) => b,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })).into_response(),
    };

    let state = state.read().await;

    // Run proof generation directly - it's fast enough (~150-200ms) and spawn_blocking/threads
    // add significant overhead due to tracing's thread-local context interfering with Rayon.
    match prove::prove_item_exists(
        &state.keys.item_exists.proving_key,
        &inventory,
        blinding,
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

// ============ Withdraw ============

#[derive(Deserialize)]
pub struct WithdrawRequest {
    pub old_inventory: Vec<SlotRequest>,
    pub old_blinding: String,
    pub new_blinding: String,
    pub item_id: u32,
    pub amount: u64,
}

#[derive(Serialize)]
pub struct WithdrawResponse {
    pub proof: String,
    pub public_inputs: Vec<String>,
    pub new_commitment: String,
}

pub async fn prove_withdraw(
    State(state): State<Arc<RwLock<AppState>>>,
    Json(req): Json<WithdrawRequest>,
) -> impl IntoResponse {
    let old_inventory = parse_inventory(&req.old_inventory);

    let old_blinding = match parse_fr(&req.old_blinding) {
        Ok(b) => b,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })).into_response(),
    };

    let new_blinding = match parse_fr(&req.new_blinding) {
        Ok(b) => b,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })).into_response(),
    };

    let state = state.read().await;

    match prove::prove_withdraw(
        &state.keys.withdraw.proving_key,
        &old_inventory,
        old_blinding,
        new_blinding,
        req.item_id,
        req.amount,
    ) {
        Ok((proof_with_inputs, _new_inventory, new_commitment)) => {
            let proof_bytes = proof_with_inputs.serialize_proof().unwrap();
            let response = WithdrawResponse {
                proof: format!("0x{}", hex::encode(proof_bytes)),
                public_inputs: proof_with_inputs
                    .public_inputs
                    .iter()
                    .map(serialize_fr)
                    .collect(),
                new_commitment: serialize_fr(&new_commitment),
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

// ============ Deposit ============

#[derive(Deserialize)]
pub struct DepositRequest {
    pub old_inventory: Vec<SlotRequest>,
    pub old_blinding: String,
    pub new_blinding: String,
    pub item_id: u32,
    pub amount: u64,
}

#[derive(Serialize)]
pub struct DepositResponse {
    pub proof: String,
    pub public_inputs: Vec<String>,
    pub new_commitment: String,
}

pub async fn prove_deposit(
    State(state): State<Arc<RwLock<AppState>>>,
    Json(req): Json<DepositRequest>,
) -> impl IntoResponse {
    let old_inventory = parse_inventory(&req.old_inventory);

    let old_blinding = match parse_fr(&req.old_blinding) {
        Ok(b) => b,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })).into_response(),
    };

    let new_blinding = match parse_fr(&req.new_blinding) {
        Ok(b) => b,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })).into_response(),
    };

    let state = state.read().await;

    match prove::prove_deposit(
        &state.keys.deposit.proving_key,
        &old_inventory,
        old_blinding,
        new_blinding,
        req.item_id,
        req.amount,
    ) {
        Ok((proof_with_inputs, _new_inventory, new_commitment)) => {
            let proof_bytes = proof_with_inputs.serialize_proof().unwrap();
            let response = DepositResponse {
                proof: format!("0x{}", hex::encode(proof_bytes)),
                public_inputs: proof_with_inputs
                    .public_inputs
                    .iter()
                    .map(serialize_fr)
                    .collect(),
                new_commitment: serialize_fr(&new_commitment),
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

// ============ Transfer ============

#[derive(Deserialize)]
pub struct TransferRequest {
    pub src_old_inventory: Vec<SlotRequest>,
    pub src_old_blinding: String,
    pub src_new_blinding: String,
    pub dst_old_inventory: Vec<SlotRequest>,
    pub dst_old_blinding: String,
    pub dst_new_blinding: String,
    pub item_id: u32,
    pub amount: u64,
}

#[derive(Serialize)]
pub struct TransferResponse {
    pub proof: String,
    pub public_inputs: Vec<String>,
    pub src_new_commitment: String,
    pub dst_new_commitment: String,
}

pub async fn prove_transfer(
    State(state): State<Arc<RwLock<AppState>>>,
    Json(req): Json<TransferRequest>,
) -> impl IntoResponse {
    let src_old = parse_inventory(&req.src_old_inventory);
    let dst_old = parse_inventory(&req.dst_old_inventory);

    let src_old_blinding = match parse_fr(&req.src_old_blinding) {
        Ok(b) => b,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })).into_response(),
    };
    let src_new_blinding = match parse_fr(&req.src_new_blinding) {
        Ok(b) => b,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })).into_response(),
    };
    let dst_old_blinding = match parse_fr(&req.dst_old_blinding) {
        Ok(b) => b,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })).into_response(),
    };
    let dst_new_blinding = match parse_fr(&req.dst_new_blinding) {
        Ok(b) => b,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })).into_response(),
    };

    let state = state.read().await;

    match prove::prove_transfer(
        &state.keys.transfer.proving_key,
        &src_old,
        src_old_blinding,
        src_new_blinding,
        &dst_old,
        dst_old_blinding,
        dst_new_blinding,
        req.item_id,
        req.amount,
    ) {
        Ok((proof_with_inputs, _, src_new_commitment, _, dst_new_commitment)) => {
            let proof_bytes = proof_with_inputs.serialize_proof().unwrap();
            let response = TransferResponse {
                proof: format!("0x{}", hex::encode(proof_bytes)),
                public_inputs: proof_with_inputs
                    .public_inputs
                    .iter()
                    .map(serialize_fr)
                    .collect(),
                src_new_commitment: serialize_fr(&src_new_commitment),
                dst_new_commitment: serialize_fr(&dst_new_commitment),
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
    pub inventory: Vec<SlotRequest>,
    pub blinding: String,
}

#[derive(Serialize)]
pub struct CreateCommitmentResponse {
    pub commitment: String,
}

pub async fn create_commitment(
    Json(req): Json<CreateCommitmentRequest>,
) -> impl IntoResponse {
    let inventory = parse_inventory(&req.inventory);

    let blinding = match parse_fr(&req.blinding) {
        Ok(b) => b,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })).into_response(),
    };

    let config = poseidon_config::<Fr>();
    let commitment = create_inventory_commitment(&inventory, blinding, &config);

    (
        StatusCode::OK,
        Json(CreateCommitmentResponse {
            commitment: serialize_fr(&commitment),
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
