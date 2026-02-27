// ./src/sse.rs

use axum::response::sse::{Event, KeepAlive, Sse};
use futures_core::Stream;
use std::convert::Infallible;
use std::ops::Deref;

// ════════════════════════════════════════════════════════════
// 1. SseRoute — typed route constant for SSE endpoints
// ════════════════════════════════════════════════════════════

/// A compile-time SSE route path. Use as both a route string and header value.
///
/// ```ignore
/// const FEED: SseRoute = SseRoute::new("/events/feed");
/// // Route:  .route(FEED.path(), get(feed_handler))
/// // Header: html(markup).sse(FEED)
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SseRoute(&'static str);

impl SseRoute {
    pub const fn new(path: &'static str) -> Self {
        Self(path)
    }

    pub const fn path(&self) -> &'static str {
        self.0
    }
}

impl Deref for SseRoute {
    type Target = str;
    fn deref(&self) -> &str {
        self.0
    }
}

impl AsRef<str> for SseRoute {
    fn as_ref(&self) -> &str {
        self.0
    }
}

// ════════════════════════════════════════════════════════════
// 2. SilcrowEvent — structured SSE event builder
// ════════════════════════════════════════════════════════════

/// A structured SSE event that Silcrow.js understands.
///
/// Two variants:
/// - `patch` — sends JSON data to be patched into a target element
/// - `html` — sends HTML markup to be swapped into a target element
#[derive(Debug)]
pub struct SilcrowEvent {
    kind: EventKind,
}

#[derive(Debug)]
enum EventKind {
    Patch {
        data: serde_json::Value,
        target: String,
    },
    Html {
        markup: String,
        target: String,
    },
}

impl SilcrowEvent {
    /// Create a patch event that sends JSON data to `Silcrow.patch(data, target)`.
    pub fn patch(data: impl serde::Serialize, target: &str) -> Self {
        let value = serde_json::to_value(data).unwrap_or_else(|e| {
            tracing::warn!("SilcrowEvent::patch serialization failed: {e}");
            serde_json::Value::Null
        });
        Self {
            kind: EventKind::Patch {
                data: value,
                target: target.to_owned(),
            },
        }
    }

    /// Create an HTML event that sends markup to `safeSetHTML(element, markup)`.
    pub fn html(markup: impl Into<String>, target: &str) -> Self {
        Self {
            kind: EventKind::Html {
                markup: markup.into(),
                target: target.to_owned(),
            },
        }
    }
}

impl From<SilcrowEvent> for Event {
    fn from(evt: SilcrowEvent) -> Event {
        match evt.kind {
            EventKind::Patch { data, target } => {
                let payload = serde_json::json!({
                    "target": target,
                    "data": data,
                });
                Event::default()
                    .event("patch")
                    .json_data(payload)
                    .unwrap_or_else(|_| Event::default().event("patch").data("{}"))
            }
            EventKind::Html { markup, target } => {
                let payload = serde_json::json!({
                    "target": target,
                    "html": markup,
                });
                Event::default()
                    .event("html")
                    .json_data(payload)
                    .unwrap_or_else(|_| Event::default().event("html").data("{}"))
            }
        }
    }
}

// ════════════════════════════════════════════════════════════
// 3. sse() — thin wrapper over Axum's Sse
// ════════════════════════════════════════════════════════════

/// Creates an SSE response from a stream of events with keep-alive enabled.
///
/// ```ignore
/// async fn feed_handler() -> impl IntoResponse {
///     let stream = stream! {
///         loop {
///             let data = get_updates().await;
///             yield Ok::<_, Infallible>(SilcrowEvent::patch(data, "#feed").into());
///         }
///     };
///     pilcrow::sse(stream)
/// }
/// ```
pub fn sse<S>(stream: S) -> Sse<S>
where
    S: Stream<Item = Result<Event, Infallible>> + Send + 'static,
{
    Sse::new(stream).keep_alive(KeepAlive::default())
}
