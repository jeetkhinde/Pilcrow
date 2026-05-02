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
///
/// Does not require `Send` because adapters are always driven from `main()`,
/// not from a spawned task.
pub type AdapterFuture = Pin<Box<dyn Future<Output = ()>>>;

/// Default adapter: binds a `tokio::net::TcpListener` with graceful shutdown.
pub struct TokioAdapter;

impl PilcrowAdapter for TokioAdapter {
    fn serve(self, bind_addr: &str, app: Router) -> AdapterFuture {
        let bind_addr = bind_addr.to_string();
        Box::pin(async move {
            let listener = match tokio::net::TcpListener::bind(&bind_addr).await {
                Ok(listener) => listener,
                Err(err) => {
                    tracing::error!(addr = %bind_addr, error = %err, "failed to bind server");
                    eprintln!("pilcrow: failed to bind {bind_addr}: {err}");
                    std::process::exit(1);
                }
            };
            tracing::info!("listening on http://{bind_addr}");
            if let Err(err) = axum::serve(listener, app)
                .with_graceful_shutdown(shutdown_signal())
                .await
            {
                tracing::error!(error = %err, "server failed");
                eprintln!("pilcrow: server failed: {err}");
                std::process::exit(1);
            }
        })
    }
}

pub(super) async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(err) = tokio::signal::ctrl_c().await {
            tracing::error!(error = %err, "failed to install Ctrl+C handler");
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let sigterm = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(err) => {
                tracing::error!(error = %err, "failed to install SIGTERM handler");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let sigterm = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = sigterm => {},
    }

    tracing::info!("shutdown signal received — draining in-flight requests");
}
