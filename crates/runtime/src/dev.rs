use std::convert::Infallible;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use axum::http::header::{CONTENT_LENGTH, CONTENT_TYPE};
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_core::Stream;

// ── SSE stream ─────────────────────────────────────────────────

/// Yields a single `custom/reload` event on connect then stays pending forever.
/// `KeepAlive` injects SSE comment frames to keep the connection alive.
/// silcrow.js reconnects with exponential backoff when the server restarts —
/// the next `custom/reload` event signals the browser to call `location.reload()`.
pub(crate) struct DevReloadStream {
    sent: bool,
}

impl Stream for DevReloadStream {
    type Item = Result<Event, Infallible>;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if !self.sent {
            self.sent = true;
            Poll::Ready(Some(Ok(Event::default()
                .event("custom")
                .data(r#"{"event":"reload","data":{}}"#))))
        } else {
            Poll::Pending
        }
    }
}

/// `GET /__pilcrow/dev-reload` — SSE endpoint for live reload in dev mode.
///
/// Sends a silcrow `custom/reload` event on every new connection. The injected
/// client script (`DEV_INJECTION`) listens for `silcrow:sse:reload` and calls
/// `location.reload()` on reconnect (i.e. after the server restarts).
pub async fn dev_reload_handler() -> Sse<DevReloadStream> {
    Sse::new(DevReloadStream { sent: false }).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("heartbeat"),
    )
}

// ── HTML injection middleware ───────────────────────────────────

/// Injected before `</body>` in every `text/html` response in dev mode.
///
/// - The hidden `<div s-sse>` opens the silcrow SSE connection. silcrow handles
///   reconnection automatically (exponential backoff 1s → 2s → 4s → … max 30s).
/// - The script listens for `silcrow:sse:reload` (fired by the custom event type).
///   `c` tracks whether we were previously connected — first connect is a no-op,
///   reconnect after server restart triggers `location.reload()`.
const DEV_INJECTION: &str = concat!(
    r#"<div id="__pilcrow_dev" s-sse="/__pilcrow/dev-reload" style="display:none"></div>"#,
    "<script>(function(){",
    "var c=false;",
    "document.addEventListener('silcrow:sse:reload',function(){",
    "if(c){location.reload();}else{c=true;}",
    "});",
    "})();</script>",
);

/// Tower middleware that appends [`DEV_INJECTION`] into `text/html` responses.
/// Non-HTML responses (JSON, SSE streams, assets) are passed through unmodified.
pub async fn dev_inject_layer(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let response = next.run(request).await;

    let is_html = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.starts_with("text/html"))
        .unwrap_or(false);

    if !is_html {
        return response;
    }

    let (mut parts, body) = response.into_parts();
    let bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(b) => b,
        Err(_) => return axum::response::Response::from_parts(parts, axum::body::Body::empty()),
    };

    let mut html = String::from_utf8_lossy(&bytes).into_owned();
    if let Some(pos) = html.rfind("</body>") {
        html.insert_str(pos, DEV_INJECTION);
    } else {
        html.push_str(DEV_INJECTION);
    }

    // Content-Length is stale after injection — remove it; transport will handle framing.
    parts.headers.remove(CONTENT_LENGTH);
    axum::response::Response::from_parts(parts, axum::body::Body::from(html))
}
