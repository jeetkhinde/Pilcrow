use axum::Router;

use crate::adapter::{shutdown_signal, AdapterFuture, PilcrowAdapter};

/// Adapter for cloud platforms that inject a `PORT` env var at runtime.
///
/// Binds to `0.0.0.0:{PORT}` when the env var is present; falls back to the
/// configured `bind_addr` from `Pilcrow.toml` otherwise. This covers:
/// Fly.io, Railway, Render, Google Cloud Run, and Heroku.
///
/// # Usage
///
/// ```rust,ignore
/// pilcrow_web::start_with_adapter(pilcrow_router(), |_| async {}, FlyAdapter).await;
/// ```
pub struct PortEnvAdapter;

/// Fly.io adapter — reads `PORT` env var, binds on all interfaces.
pub type FlyAdapter = PortEnvAdapter;
/// Railway adapter — reads `PORT` env var, binds on all interfaces.
pub type RailwayAdapter = PortEnvAdapter;
/// Google Cloud Run adapter — reads `PORT` env var, binds on all interfaces.
pub type CloudRunAdapter = PortEnvAdapter;
/// Render adapter — reads `PORT` env var, binds on all interfaces.
pub type RenderAdapter = PortEnvAdapter;
/// Vercel long-running (non-Lambda) adapter — reads `PORT` env var.
pub type VercelAdapter = PortEnvAdapter;

impl PilcrowAdapter for PortEnvAdapter {
    fn serve(self, bind_addr: &str, app: Router) -> AdapterFuture {
        let addr = match std::env::var("PORT") {
            Ok(port) => format!("0.0.0.0:{port}"),
            Err(_) => bind_addr.to_string(),
        };
        Box::pin(async move {
            let listener = tokio::net::TcpListener::bind(&addr)
                .await
                .unwrap_or_else(|e| panic!("Failed to bind to {addr}: {e}"));
            tracing::info!("listening on http://{addr}");
            axum::serve(listener, app)
                .with_graceful_shutdown(shutdown_signal())
                .await
                .expect("serve");
        })
    }
}
