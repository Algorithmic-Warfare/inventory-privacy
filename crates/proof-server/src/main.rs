//! HTTP API server for inventory proof generation.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};

mod handlers;
mod routes;

use inventory_prover::setup::{setup_all_circuits, CircuitKeys};

/// Application state shared across handlers
pub struct AppState {
    pub keys: Arc<CircuitKeys>,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() {
    // CRITICAL: tracing_subscriber causes 400x performance regression with Rayon!
    // When enabled, proof generation takes 6+ seconds instead of 15-20ms.
    // Issue: tracing's thread-local context interferes with Rayon's work-stealing.
    // Using println! instead of tracing for server startup messages.

    println!("Starting inventory proof server...");

    // Load or generate circuit keys
    let keys_dir = std::path::Path::new("keys");
    let keys = if keys_dir.exists() {
        println!("Loading existing circuit keys from {:?}", keys_dir);
        CircuitKeys::load_from_directory(keys_dir).expect("Failed to load circuit keys")
    } else {
        println!("Running trusted setup (this may take a while)...");
        let keys = setup_all_circuits().expect("Failed to setup circuits");
        keys.save_to_directory(keys_dir)
            .expect("Failed to save circuit keys");
        println!("Circuit keys saved to {:?}", keys_dir);
        keys
    };

    let state = Arc::new(RwLock::new(AppState { keys: Arc::new(keys) }));

    // Build router
    let app = Router::new()
        .merge(routes::api_routes())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state);

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
