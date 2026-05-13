use axum::{
    extract::Query,
    response::sse::{Event, KeepAlive, Sse},
    response::IntoResponse,
    Extension,
};
use serde::Deserialize;
use std::convert::Infallible;
use std::sync::Arc;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt as _;

use super::watcher::WatcherEventTx;

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
                let event = Event::default()
                    .event("fsr")
                    .data(payload.to_string());
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
    use axum::http::StatusCode;

    match ext {
        Some(tx) => fsr_hub_handler(query, tx).await.into_response(),
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            "FSR not configured (DATABASE_URL missing)",
        )
            .into_response(),
    }
}
