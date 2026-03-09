use axum::response::sse::{Event, KeepAlive, Sse};
use futures_core::Stream;
use std::convert::Infallible;
use std::future::Future;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;

crate::define_route!(SseRoute, "SSE", "/events/feed", "FEED");

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
pub(crate) enum EventKind {
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
        let value = super::macros::serialize_or_null(data, "SilcrowEvent::patch");
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
    /// Create a JSON event. Wire format is identical to `patch`.
    pub fn json(data: impl serde::Serialize, target: &str) -> Self {
        let value = super::macros::serialize_or_null(data, "SilcrowEvent::json");
        Self {
            kind: EventKind::Patch {
                data: value,
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

/// Returned by `SseEmitter::send` when the client has disconnected.
/// Handlers should treat this as a signal to stop their loop.
#[derive(Debug)]
pub struct EmitError;

impl std::fmt::Display for EmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SSE client disconnected")
    }
}

impl std::error::Error for EmitError {}

/// A handle passed to `sse()` closure handlers for sending events to the client.
///
/// Returns `Err(EmitError)` when the client disconnects — use this to terminate loops cleanly.
///
/// ```ignore
/// pilcrow::sse_stream(|emit| async move {
///     loop {
///         let data = fetch().await;
///         emit.send(SilcrowEvent::patch(&data, "#feed")).await?;
///         tokio::time::sleep(Duration::from_secs(5)).await;
///     }
///     Ok(())
/// })
/// ```
#[derive(Clone)]
pub struct SseEmitter {
    tx: mpsc::Sender<SilcrowEvent>,
}

impl SseEmitter {
    /// Send a `SilcrowEvent` to the connected client.
    ///
    /// Returns `Err(EmitError)` if the client has disconnected (receiver dropped).
    /// This is the signal to break out of a loop.
    pub async fn send(&self, event: SilcrowEvent) -> Result<(), EmitError> {
        self.tx.send(event).await.map_err(|_| EmitError)
    }
    /// Convenience for sending serializable data to a DOM target.
    pub async fn json(&self, target: &str, data: &impl serde::Serialize) -> Result<(), EmitError> {
        self.send(SilcrowEvent::json(data, target)).await
    }
}

/// Creates an SSE response from a closure that receives an `SseEmitter`.
///
/// This is the primary API. The framework manages channel creation, task spawning,
/// and connection lifecycle. When the client disconnects, `emit.send()` returns
/// `Err(EmitError)` — use `?` to propagate and terminate the handler cleanly.
///
/// ```ignore
/// pub async fn handler(Extension(state): Extension<AppState>) -> impl IntoResponse {
///     pilcrow::sse_stream(|emit| async move {
///         loop {
///             let data = state.fetch().await;
///             emit.send(SilcrowEvent::patch(&data, "#feed")).await?;
///             tokio::time::sleep(Duration::from_secs(5)).await;
///         }
///         #[allow(unreachable_code)]
///         Ok(())
///     })
/// }
/// ```
pub fn sse_stream<F, Fut>(
    handler: F,
) -> Sse<impl Stream<Item = Result<Event, Infallible>> + Send + 'static>
where
    F: FnOnce(SseEmitter) -> Fut + Send + 'static,
    Fut: Future<Output = Result<(), EmitError>> + Send + 'static,
{
    let (tx, rx) = mpsc::channel::<SilcrowEvent>(32);
    let emitter = SseEmitter { tx };

    tokio::spawn(async move {
        let _ = handler(emitter).await;
    });

    let stream = ReceiverStream::new(rx).map(|event| Ok::<Event, Infallible>(event.into()));

    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// Creates an SSE response from a pre-composed stream of events.
///
/// Use this for advanced composition with `futures_util` / `tokio-stream` combinators.
/// For most use cases, prefer `sse()` which provides better lifecycle management.
///
/// ```ignore
/// async fn handler() -> impl IntoResponse {
///     let stream = async_stream::stream! {
///         loop {
///             let data = fetch().await;
///             yield Ok::<_, Infallible>(SilcrowEvent::patch(&data, "#feed").into());
///         }
///     };
///     pilcrow::sse_raw(stream)
/// }
/// ```
pub fn sse_raw<S>(stream: S) -> Sse<S>
where
    S: Stream<Item = Result<Event, Infallible>> + Send + 'static,
{
    Sse::new(stream).keep_alive(KeepAlive::default())
}
