use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use axum::error_handling::HandleErrorLayer;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::BoxError;
use axum::Router;
use pilcrow_core::config::config::CacheProvider;
use pilcrow_core::PilcrowConfig;
use tower::timeout::TimeoutLayer;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;

use crate::isr::{IsrCache, IsrHandle};

const REQUEST_TIMEOUT_SECS: u64 = 30;

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

    // Build the ISR cache, optionally with filesystem persistence.
    let isr_cache = Arc::new(match &config.cache.provider {
        CacheProvider::Filesystem => {
            let dir = config.cache.dir.as_deref().unwrap_or(".pilcrow-cache");
            tracing::info!("ISR cache: filesystem backend at {dir}");
            IsrCache::with_persistence(dir)
        }
        _ => IsrCache::new(),
    });

    prerender_fn(Arc::clone(&isr_cache)).await;
    let isr_handle = IsrHandle::new(Arc::clone(&isr_cache));

    let app = app
        // Dev inspection endpoint — returns JSON snapshot of the ISR cache.
        .route("/__pilcrow/isr", axum::routing::get(isr_inspect_handler))
        .layer(axum::Extension(config))
        .layer(axum::Extension(http))
        .layer(axum::Extension(isr_handle))
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(|err: BoxError| async move {
                    if err.is::<tower::timeout::error::Elapsed>() {
                        StatusCode::REQUEST_TIMEOUT
                    } else {
                        StatusCode::INTERNAL_SERVER_ERROR
                    }
                }))
                .layer(TimeoutLayer::new(Duration::from_secs(REQUEST_TIMEOUT_SECS))),
        )
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .unwrap_or_else(|e| panic!("Failed to bind to {bind_addr}: {e}"));

    tracing::info!("listening on http://{bind_addr}");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("serve");
}

/// `GET /__pilcrow/isr` — returns a JSON snapshot of the ISR cache.
///
/// Useful in development to inspect cache state, TTLs, tags, and revalidation status.
/// The response is always `application/json`; returns an empty array when the cache
/// has no entries.
async fn isr_inspect_handler(
    axum::Extension(handle): axum::Extension<IsrHandle>,
) -> axum::response::Response {
    match handle.__arc() {
        Some(cache) => axum::Json(cache.snapshot()).into_response(),
        None => (StatusCode::OK, axum::Json(serde_json::json!([]))).into_response(),
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install Ctrl+C handler");
    };

    #[cfg(unix)]
    let sigterm = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let sigterm = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = sigterm => {},
    }

    tracing::info!("shutdown signal received — draining in-flight requests");
}
