use std::future::Future;
use std::pin::Pin;

use axum::Router;

/// Pluggable deployment adapter.
///
/// Receives the fully-wired `Router` (ISR + middleware layers applied) and the
/// `bind_addr` from `Pilcrow.toml`, and is responsible for binding and serving.
///
/// The default [`TokioAdapter`] binds a `tokio::net::TcpListener` with graceful
/// shutdown on SIGTERM / Ctrl-C.
///
/// # Custom adapter example
///
/// ```rust,ignore
/// struct LambdaAdapter;
///
/// impl PilcrowAdapter for LambdaAdapter {
///     fn serve(self, _bind_addr: &str, app: Router) -> AdapterFuture {
///         Box::pin(async move {
///             lambda_http::run(app).await.expect("lambda serve");
///         })
///     }
/// }
///
/// // In main.rs:
/// pilcrow_web::start_with_adapter(pilcrow_router(), |_| async {}, LambdaAdapter).await;
/// ```
pub trait PilcrowAdapter: Send + 'static {
    fn serve(self, bind_addr: &str, app: Router) -> AdapterFuture;
}

/// Boxed pinned future returned by [`PilcrowAdapter::serve`].
pub type AdapterFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

/// Default adapter: binds a `tokio::net::TcpListener` with graceful shutdown.
pub struct TokioAdapter;

impl PilcrowAdapter for TokioAdapter {
    fn serve(self, bind_addr: &str, app: Router) -> AdapterFuture {
        let bind_addr = bind_addr.to_string();
        Box::pin(async move {
            let listener = tokio::net::TcpListener::bind(&bind_addr)
                .await
                .unwrap_or_else(|e| panic!("Failed to bind to {bind_addr}: {e}"));
            tracing::info!("listening on http://{bind_addr}");
            axum::serve(listener, app)
                .with_graceful_shutdown(shutdown_signal())
                .await
                .expect("serve");
        })
    }
}

pub(super) async fn shutdown_signal() {
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
