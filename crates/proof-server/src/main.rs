//! HTTP API server for inventory proof generation.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod handlers;
mod routes;

use inventory_prover::setup::{setup_all_circuits, CircuitKeys};

/// Application state shared across handlers
pub struct AppState {
    pub keys: CircuitKeys,
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting inventory proof server...");

    // Load or generate circuit keys
    let keys_dir = std::path::Path::new("keys");
    let keys = if keys_dir.exists() {
        tracing::info!("Loading existing circuit keys from {:?}", keys_dir);
        CircuitKeys::load_from_directory(keys_dir).expect("Failed to load circuit keys")
    } else {
        tracing::info!("Running trusted setup (this may take a while)...");
        let keys = setup_all_circuits().expect("Failed to setup circuits");
        keys.save_to_directory(keys_dir)
            .expect("Failed to save circuit keys");
        tracing::info!("Circuit keys saved to {:?}", keys_dir);
        keys
    };

    let state = Arc::new(RwLock::new(AppState { keys }));

    // Build router
    let app = Router::new()
        .merge(routes::api_routes())
        .layer(TraceLayer::new_for_http())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state);

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
