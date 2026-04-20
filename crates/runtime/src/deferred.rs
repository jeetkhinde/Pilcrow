use std::fmt;
use std::future::Future;
use std::pin::Pin;

use axum::body::Body;
use axum::response::{IntoResponse, Response};
use futures_core::Stream;
use serde::Serialize;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

/// A lazily-resolved value that the framework can stream to the client after the shell renders.
///
/// Declare `Deferred<T>` fields on a `Props` struct to enable streaming:
///
/// ```rust,ignore
/// pub struct Props {
///     pub title: String,
///     pub count: Deferred<i32>,
/// }
///
/// pub async fn load(_req: Req) -> AppResult<Props> {
///     Ok(Props {
///         title: "Hello".to_string(),
///         count: Deferred::spawn(async {
///             tokio::time::sleep(std::time::Duration::from_millis(200)).await;
///             42
///         }),
///     })
/// }
/// ```
///
/// The framework renders the shell HTML immediately (with `Deferred` fields as `""`), then
/// streams `<script>window.__pilcrow_deferred('field_name', value)</script>` patches as each
/// future resolves.
pub struct Deferred<T> {
    inner: DeferredInner<T>,
}

enum DeferredInner<T> {
    Future(Pin<Box<dyn Future<Output = T> + Send + 'static>>),
    Ready(T),
}

impl<T: Serialize + Send + 'static> Deferred<T> {
    /// Create a deferred value backed by a future. Starts resolving when awaited.
    pub fn spawn(fut: impl Future<Output = T> + Send + 'static) -> Self {
        Self { inner: DeferredInner::Future(Box::pin(fut)) }
    }

    /// Create an already-resolved deferred value.
    pub fn ready(value: T) -> Self {
        Self { inner: DeferredInner::Ready(value) }
    }

    /// Resolve the deferred value. Consumes self.
    pub async fn resolve(self) -> T {
        match self.inner {
            DeferredInner::Future(fut) => fut.await,
            DeferredInner::Ready(v) => v,
        }
    }
}

/// Renders as an empty string — the loading placeholder in the shell template.
impl<T> fmt::Display for Deferred<T> {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}

impl<T: fmt::Debug> fmt::Debug for Deferred<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner {
            DeferredInner::Ready(v) => write!(f, "Deferred::Ready({v:?})"),
            DeferredInner::Future(_) => write!(f, "Deferred::Future(...)"),
        }
    }
}

impl<T: Serialize> serde::Serialize for Deferred<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match &self.inner {
            DeferredInner::Ready(v) => v.serialize(serializer),
            DeferredInner::Future(_) => serializer.serialize_str(""),
        }
    }
}

/// A (field_name, serialized_json_value) pair streamed after the shell renders.
pub struct DeferredPatch {
    pub field: &'static str,
    pub json: String,
}

/// Build a streaming `Response` for pages with deferred fields.
///
/// - `shell_html`: the fully-rendered shell (deferred fields show as empty strings).
/// - `patches`: an async iterator of `DeferredPatch` values that will be streamed
///   as inline `<script>` chunks after the shell.
pub fn deferred_response(
    shell_html: String,
    patches: impl Stream<Item = DeferredPatch> + Send + 'static,
) -> Response {
    use tokio_stream::StreamExt as _;

    let (tx, rx) = mpsc::channel::<Result<bytes::Bytes, std::convert::Infallible>>(8);

    tokio::spawn(async move {
        let shell_bytes = bytes::Bytes::from(shell_html);
        if tx.send(Ok(shell_bytes)).await.is_err() {
            return;
        }
        tokio::pin!(patches);
        while let Some(patch) = patches.next().await {
            let chunk = format!(
                "<script>window.__pilcrow_deferred({name},{value})</script>",
                name = serde_json::to_string(patch.field).unwrap_or_default(),
                value = patch.json,
            );
            if tx.send(Ok(bytes::Bytes::from(chunk))).await.is_err() {
                return;
            }
        }
    });

    let stream = ReceiverStream::new(rx);
    let body = Body::from_stream(stream);

    axum::http::Response::builder()
        .header("content-type", "text/html; charset=utf-8")
        .body(body)
        .expect("valid response")
}

/// Serialize a resolved deferred value to JSON string, or `"null"` on error.
#[doc(hidden)]
pub fn __serialize_deferred<T: Serialize>(value: T) -> String {
    serde_json::to_string(&value).unwrap_or_else(|_| "null".to_string())
}

/// Build a `Stream<Item = DeferredPatch>` from a vec of `(field, future → String)` pairs.
/// Resolves patches concurrently and yields them in completion order.
#[doc(hidden)]
pub fn __deferred_patch_stream(
    pairs: Vec<(&'static str, Pin<Box<dyn Future<Output = String> + Send>>)>,
) -> impl Stream<Item = DeferredPatch> + Send {
    let (tx, rx) = mpsc::channel::<DeferredPatch>(pairs.len().max(1));
    for (field, fut) in pairs {
        let tx = tx.clone();
        tokio::spawn(async move {
            let json = fut.await;
            let _ = tx.send(DeferredPatch { field, json }).await;
        });
    }
    ReceiverStream::new(rx)
}
