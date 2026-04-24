use axum::Router;
use pilcrow_core::PilcrowConfig;
use std::future::Future;
use std::sync::Arc;

use crate::isr::{IsrCache, IsrHandle};

pub async fn start(app: Router) {
    start_with_prerender(app, |_cache| async {}).await;
}

/// Start the Pilcrow web server, calling `prerender_fn` with the shared `IsrCache`
/// before accepting connections. Used by SSG apps to pre-warm the cache at startup.
///
/// ```ignore
/// pilcrow_web::start_with_prerender(app, |cache| async move {
///     __pilcrow_app::__pilcrow_prerender_all(&cache).await
/// }).await;
/// ```
pub async fn start_with_prerender<F, Fut>(app: Router, prerender_fn: F)
where
    F: FnOnce(Arc<IsrCache>) -> Fut,
    Fut: Future<Output = ()>,
{
    let config = Arc::new(PilcrowConfig::load_from_current_dir().expect("load Pilcrow.toml"));
    let bind_addr = config.web_bind_addr();
    let http = reqwest::Client::new();

    // Initialise the ISR cache and make it available to all request handlers
    // via `req.cache`. A single IsrCache instance is shared across all requests.
    let isr_cache = Arc::new(IsrCache::new());

    // Pre-render SSG pages before accepting connections.
    prerender_fn(Arc::clone(&isr_cache)).await;

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
