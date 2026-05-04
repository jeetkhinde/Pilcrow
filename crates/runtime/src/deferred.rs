use std::fmt;
use std::future::Future;
use std::pin::Pin;

use axum::body::Body;
use axum::http::StatusCode;
use axum::response::Response;
use futures_core::Stream;
use futures_util::stream::FuturesUnordered;
use serde::Serialize;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

fn html_stream_response(body: Body) -> Response {
    match axum::http::Response::builder()
        .header("content-type", "text/html; charset=utf-8")
        .header("silcrow-full-reload", "true")
        .body(body)
    {
        Ok(response) => response,
        Err(err) => {
            tracing::error!(error = %err, "failed to build deferred response");
            let mut response = Response::new(Body::from("internal server error"));
            *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
            response
        }
    }
}

// ── DeferredHtml ─────────────────────────────────────────────────────────────

/// A lazily-resolved HTML fragment that streams into a keyed slot after the shell renders.
///
/// Use `DeferredHtml` when you need to stream complex markup (lists, cards, nested HTML) rather
/// than scalar values. In the template, `{{ field }}` renders as a `<span data-pilcrow-slot>`
/// placeholder; the resolved HTML replaces it once the future completes.
///
/// ```rust,ignore
/// pub struct Props {
///     pub title: String,
///     pub products: DeferredHtml,
/// }
///
/// pub async fn load(_req: Req) -> AppResult<Props> {
///     use fragments::widgets::product_list;
///     Ok(Props {
///         title: "Products".into(),
///         products: DeferredHtml::spawn(async {
///             let items = db::get_products().await;
///             product_list::render(product_list::Props { items }).unwrap_or_default()
///         }),
///     })
/// }
/// ```
///
/// Add a loading skeleton shown while the future resolves:
/// ```rust,ignore
/// DeferredHtml::spawn(async { ... }).with_loading("<div class='skeleton'></div>")
/// ```
pub struct DeferredHtml {
    inner: DeferredHtmlInner,
    pub loading: String,
}

enum DeferredHtmlInner {
    Future(Pin<Box<dyn Future<Output = String> + Send + 'static>>),
    Slot(String),
}

impl DeferredHtml {
    /// Create a deferred HTML slot backed by a future that resolves to an HTML string.
    pub fn spawn(fut: impl Future<Output = String> + Send + 'static) -> Self {
        Self {
            inner: DeferredHtmlInner::Future(Box::pin(fut)),
            loading: String::new(),
        }
    }

    /// Set the loading skeleton shown in the slot while the future is resolving.
    pub fn with_loading(mut self, html: impl Into<String>) -> Self {
        self.loading = html.into();
        self
    }

    /// Called by generated code: extract the future and loading HTML for streaming.
    #[doc(hidden)]
    pub fn __into_parts(
        self,
    ) -> (
        Pin<Box<dyn Future<Output = String> + Send + 'static>>,
        String,
    ) {
        match self.inner {
            DeferredHtmlInner::Future(f) => (f, self.loading),
            DeferredHtmlInner::Slot(_) => {
                tracing::error!("DeferredHtml::__into_parts called on a slot placeholder");
                (Box::pin(async { String::new() }), self.loading)
            }
        }
    }

    /// Called by generated code: create a placeholder used during shell rendering.
    #[doc(hidden)]
    pub fn __slot(name: impl Into<String>, loading: String) -> Self {
        Self {
            inner: DeferredHtmlInner::Slot(name.into()),
            loading,
        }
    }
}

/// Renders as a unique text marker that generated code replaces with the actual slot span.
///
/// The marker `__pilcrow_html_slot_{name}__` contains no HTML-special characters so Askama
/// will not escape it, making a simple string-replace safe.
impl fmt::Display for DeferredHtml {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner {
            DeferredHtmlInner::Slot(name) => write!(f, "__pilcrow_html_slot_{name}__"),
            DeferredHtmlInner::Future(_) => Ok(()),
        }
    }
}

impl fmt::Debug for DeferredHtml {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner {
            DeferredHtmlInner::Slot(n) => write!(f, "DeferredHtml::Slot({n:?})"),
            DeferredHtmlInner::Future(_) => write!(f, "DeferredHtml::Future(...)"),
        }
    }
}

impl serde::Serialize for DeferredHtml {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str("")
    }
}

/// A resolved HTML patch streamed into a named slot.
pub struct DeferredHtmlPatch {
    pub slot: &'static str,
    pub html: String,
}

/// Build a `Stream<Item = DeferredHtmlPatch>` from (slot, future → html) pairs.
/// Resolves concurrently and yields in completion order.
#[doc(hidden)]
pub fn __deferred_html_patch_stream(
    pairs: Vec<(&'static str, Pin<Box<dyn Future<Output = String> + Send>>)>,
) -> impl Stream<Item = DeferredHtmlPatch> + Send {
    pairs
        .into_iter()
        .map(|(slot, fut)| async move {
            let html = fut.await;
            DeferredHtmlPatch { slot, html }
        })
        .collect::<FuturesUnordered<_>>()
}

/// Build a streaming `Response` for pages with both `Deferred<T>` (JSON) and `DeferredHtml` fields.
///
/// Resolves all futures concurrently; JSON patches call `window.__pilcrow_deferred`,
/// HTML patches call `window.__pd` to swap the slot span.
pub fn deferred_response_combined(
    shell_html: String,
    json_patches: impl Stream<Item = DeferredPatch> + Send + 'static,
    html_patches: impl Stream<Item = DeferredHtmlPatch> + Send + 'static,
) -> Response {
    use futures_util::StreamExt as _;

    let (tx, rx) = mpsc::channel::<Result<bytes::Bytes, std::convert::Infallible>>(8);

    tokio::spawn(async move {
        if tx.send(Ok(bytes::Bytes::from(shell_html))).await.is_err() {
            return;
        }

        let json_tx = tx.clone();
        let json_task = tokio::spawn(async move {
            tokio::pin!(json_patches);
            while let Some(p) = json_patches.next().await {
                let chunk = format!(
                    "<script>window.__pilcrow_deferred({name},{value})</script>",
                    name = serde_json::to_string(p.field).unwrap_or_default(),
                    value = p.json,
                );
                if json_tx.send(Ok(bytes::Bytes::from(chunk))).await.is_err() {
                    break;
                }
            }
        });

        let html_tx = tx;
        let html_task = tokio::spawn(async move {
            tokio::pin!(html_patches);
            while let Some(p) = html_patches.next().await {
                let chunk = format!(
                    "<script>window.__pd({slot},{html})</script>",
                    slot = serde_json::to_string(p.slot).unwrap_or_default(),
                    html = serde_json::to_string(&p.html).unwrap_or_default(),
                );
                if html_tx.send(Ok(bytes::Bytes::from(chunk))).await.is_err() {
                    break;
                }
            }
        });

        let _ = tokio::join!(json_task, html_task);
    });

    let stream = ReceiverStream::new(rx);
    let body = Body::from_stream(stream);

    html_stream_response(body)
}

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
        Self {
            inner: DeferredInner::Future(Box::pin(fut)),
        }
    }

    /// Create an already-resolved deferred value.
    pub fn ready(value: T) -> Self {
        Self {
            inner: DeferredInner::Ready(value),
        }
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
    use futures_util::StreamExt as _;

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

    html_stream_response(body)
}

/// Serialize a resolved deferred value to JSON string, or `"null"` on error.
#[doc(hidden)]
pub fn __serialize_deferred<T: Serialize>(value: T) -> String {
    serde_json::to_string(&value).unwrap_or_else(|_| "null".to_string())
}

/// Build a streaming Response for `STREAMING = true` pages.
///
/// Sends the shell HTML immediately, then awaits the props JSON future and streams
/// a single `window.__ps(json)` call that lets silcrow.js patch all page bindings at once.
/// If the future returns an empty string (load errored or task panicked), no patch is emitted.
#[doc(hidden)]
pub fn __streaming_props_response(
    shell_html: String,
    props_json_future: impl Future<Output = String> + Send + 'static,
) -> Response {
    let (tx, rx) = mpsc::channel::<Result<bytes::Bytes, std::convert::Infallible>>(2);

    tokio::spawn(async move {
        if tx.send(Ok(bytes::Bytes::from(shell_html))).await.is_err() {
            return;
        }
        let json = props_json_future.await;
        if !json.is_empty() {
            let chunk = format!("<script>window.__ps({})</script>", json);
            let _ = tx.send(Ok(bytes::Bytes::from(chunk))).await;
        }
    });

    let stream = ReceiverStream::new(rx);
    let body = Body::from_stream(stream);

    html_stream_response(body)
}

/// Serialize page Props to a JSON object string for STREAMING patches.
///
/// Returns an empty string on serialization failure (treated as "no patch" by
/// `__streaming_props_response`).
#[doc(hidden)]
pub fn __serialize_page_props<T: Serialize>(value: T) -> String {
    serde_json::to_string(&value).unwrap_or_default()
}

/// Build a `Stream<Item = DeferredPatch>` from a vec of `(field, future → String)` pairs.
/// Resolves patches concurrently and yields them in completion order.
#[doc(hidden)]
pub fn __deferred_patch_stream(
    pairs: Vec<(&'static str, Pin<Box<dyn Future<Output = String> + Send>>)>,
) -> impl Stream<Item = DeferredPatch> + Send {
    pairs
        .into_iter()
        .map(|(field, fut)| async move {
            let json = fut.await;
            DeferredPatch { field, json }
        })
        .collect::<FuturesUnordered<_>>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    use std::task::{Context, Poll};

    struct PendingUntilDropped {
        dropped: Arc<AtomicBool>,
    }

    impl Future for PendingUntilDropped {
        type Output = String;

        fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
            Poll::Pending
        }
    }

    impl Drop for PendingUntilDropped {
        fn drop(&mut self) {
            self.dropped.store(true, Ordering::SeqCst);
        }
    }

    #[test]
    fn dropping_deferred_patch_stream_drops_pending_future() {
        let dropped = Arc::new(AtomicBool::new(false));
        let stream = __deferred_patch_stream(vec![(
            "count",
            Box::pin(PendingUntilDropped {
                dropped: Arc::clone(&dropped),
            }),
        )]);

        drop(stream);

        assert!(dropped.load(Ordering::SeqCst));
    }
}
