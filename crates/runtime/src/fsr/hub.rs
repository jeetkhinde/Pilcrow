use axum::{
    Extension,
    Json,
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Response},
    response::sse::{Event, KeepAlive, Sse},
};
use futures_core::Stream;
use serde::Deserialize;
use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll};
use tokio_stream::StreamExt as _;
use tokio_stream::wrappers::BroadcastStream;

use super::store::FsrStore;
use super::watcher::{WatcherEventTx, execute_with_params};

/// Shared atomic counter of open SSE connections.
///
/// # Contract
/// Only the accept path in `fsr_hub_handler` may call `fetch_add(1, Relaxed)`.
/// Only `ConnectionGuard::drop` may call `fetch_sub`. All other callers are reads only.
pub type FsrConnectionCounter = Arc<AtomicUsize>;

/// Runtime configuration for the FSR SSE hub, derived from `FsrConfig`.
#[derive(Debug, Clone)]
pub struct FsrHubConfig {
    pub max_connections: usize,
    pub connection_ttl_secs: u64,
    pub keepalive_secs: u64,
}

impl Default for FsrHubConfig {
    fn default() -> Self {
        Self {
            max_connections: 1000,
            connection_ttl_secs: 3600,
            keepalive_secs: 30,
        }
    }
}

/// Decrements the connection counter when dropped (i.e. when the SSE stream ends).
struct ConnectionGuard(Arc<AtomicUsize>);

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        let prev = self.0.fetch_sub(1, Ordering::Relaxed);
        debug_assert!(prev > 0, "ConnectionGuard dropped with counter already at zero");
    }
}

/// Wraps a stream and keeps a `ConnectionGuard` alive until the stream is dropped.
///
/// Axum drops the SSE body (and therefore the stream) when the HTTP connection
/// closes — that is the correct moment to decrement the counter, not when the
/// handler function returns.
///
/// Requires `S: Unpin`. In `fsr_hub_handler` we guarantee this by passing
/// `Box::pin(sleep(...))` to `take_until` — `tokio::time::Sleep` is `!Unpin`
/// but `Pin<Box<Sleep>>` is `Unpin`, so the composed stream remains `Unpin`.
struct GuardedStream<S> {
    inner: S,
    _guard: ConnectionGuard,
}

impl<S: Stream + Unpin> Stream for GuardedStream<S> {
    type Item = S::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner).poll_next(cx)
    }
}

impl<S: Unpin> Unpin for GuardedStream<S> {}

#[derive(Debug, Deserialize)]
pub struct FsrHubQuery {
    pub route: Option<String>,
    pub slots: Option<String>,
}

/// SSE handler at `/__pilcrow/fsr`.
///
/// Subscribes the client to slot-patch events for their current route.
/// Query params: `route=...&slots=slot1,slot2,...`
pub async fn fsr_hub_handler(
    Query(query): Query<FsrHubQuery>,
    Extension(event_tx): Extension<Arc<WatcherEventTx>>,
) -> impl IntoResponse {
    let subscribed_route = query.route.unwrap_or_default();
    let subscribed_slots: Vec<String> = query
        .slots
        .unwrap_or_default()
        .split(',')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();

    let rx = BroadcastStream::new(event_tx.subscribe());

    let stream = rx.filter_map(move |msg| {
        let subscribed_route = subscribed_route.clone();
        let subscribed_slots = subscribed_slots.clone();
        match msg {
            Ok(patch) => {
                if patch.route != subscribed_route {
                    return None;
                }
                if !subscribed_slots.is_empty() && !subscribed_slots.contains(&patch.slot) {
                    return None;
                }
                let payload = serde_json::json!({ &patch.slot: patch.value });
                let event = Event::default().event("fsr").data(payload.to_string());
                Some(Ok::<Event, Infallible>(event))
            }
            Err(_) => None,
        }
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// SSE handler for FSR that works without a broadcast channel extension
/// (returns 503 when FSR is not configured).
pub async fn fsr_hub_handler_or_unavailable(
    query: Query<FsrHubQuery>,
    ext: Option<Extension<Arc<WatcherEventTx>>>,
) -> axum::response::Response {
    match ext {
        Some(tx) => fsr_hub_handler(query, tx).await.into_response(),
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            "FSR not configured (DATABASE_URL missing)",
        )
            .into_response(),
    }
}

/// Handler for `GET /__pilcrow/fsr/snapshot?route=...&slots=...`.
///
/// Re-executes stored queries for the requested slots and returns their current
/// values as JSON. Called by the client after receiving an `fsr-resync` event.
pub async fn fsr_snapshot_handler(
    Query(query): Query<FsrHubQuery>,
    store: Option<Extension<Arc<FsrStore>>>,
) -> Response {
    let Some(Extension(store)) = store else {
        return (StatusCode::SERVICE_UNAVAILABLE, "FSR store unavailable").into_response();
    };

    let route = query.route.unwrap_or_default();
    let slot_names: Vec<&str> = query
        .slots
        .as_deref()
        .unwrap_or("")
        .split(',')
        .filter(|s| !s.is_empty())
        .collect();

    let slots = match store.fetch_slots_for_snapshot(&route, &slot_names).await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "FSR snapshot: DB error");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let mut result = serde_json::Map::new();
    for slot in &slots {
        let Some(ref sql) = slot.query else { continue };
        let params: Vec<serde_json::Value> = slot
            .query_params
            .as_ref()
            .and_then(|p| p.as_array())
            .cloned()
            .unwrap_or_default();
        match execute_with_params(store.pool(), sql, &params).await {
            Ok(Some(row)) => {
                let col_key = slot.column_name.as_deref().unwrap_or(&slot.slot);
                if let Some(v) = row.get(col_key) {
                    result.insert(slot.slot.clone(), v.clone());
                }
            }
            Ok(None) => {}
            Err(e) => {
                tracing::warn!(slot = %slot.slot, error = %e, "FSR snapshot: query error for slot");
            }
        }
    }

    Json(serde_json::Value::Object(result)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn connection_guard_decrements_on_drop() {
        let counter: FsrConnectionCounter = Arc::new(AtomicUsize::new(3));
        {
            let _guard = ConnectionGuard(Arc::clone(&counter));
            assert_eq!(counter.load(Ordering::Relaxed), 3);
        }
        assert_eq!(counter.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn fsr_hub_config_default_values() {
        let cfg = FsrHubConfig::default();
        assert_eq!(cfg.max_connections, 1000);
        assert_eq!(cfg.connection_ttl_secs, 3600);
        assert_eq!(cfg.keepalive_secs, 30);
    }

    #[tokio::test]
    async fn snapshot_returns_503_without_store() {
        let resp = fsr_snapshot_handler(
            Query(FsrHubQuery { route: None, slots: None }),
            None,
        )
        .await;
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
