use axum::Router;
use pilcrow_core::PilcrowConfig;
use std::sync::Arc;

use crate::isr::{IsrCache, IsrHandle};

pub async fn start(app: Router) {
    let config = Arc::new(PilcrowConfig::load_from_current_dir().expect("load Pilcrow.toml"));
    let bind_addr = config.web_bind_addr();
    let http = reqwest::Client::new();

    // Initialise the ISR cache and make it available to all request handlers
    // via `req.cache`. A single IsrCache instance is shared across all requests.
    let isr_cache = Arc::new(IsrCache::new());
    let isr_handle = IsrHandle::new(Arc::clone(&isr_cache));

    let app = app
        .layer(axum::Extension(config))
        .layer(axum::Extension(http))
        .layer(axum::Extension(isr_handle));

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .unwrap_or_else(|e| panic!("Failed to bind to {bind_addr}: {e}"));

    tracing::info!("listening on http://{bind_addr}");
    axum::serve(listener, app).await.expect("serve");
}
