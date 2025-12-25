//! API route definitions.

use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};
use tokio::sync::RwLock;

use crate::handlers;
use crate::AppState;

/// Create API routes
pub fn api_routes() -> Router<Arc<RwLock<AppState>>> {
    Router::new()
        // Health check
        .route("/health", get(handlers::health))
        // Proof generation endpoints
        .route("/api/prove/item-exists", post(handlers::prove_item_exists))
        .route("/api/prove/withdraw", post(handlers::prove_withdraw))
        .route("/api/prove/deposit", post(handlers::prove_deposit))
        .route("/api/prove/transfer", post(handlers::prove_transfer))
        // Utility endpoints
        .route("/api/commitment/create", post(handlers::create_commitment))
        .route("/api/blinding/generate", post(handlers::generate_blinding))
}
