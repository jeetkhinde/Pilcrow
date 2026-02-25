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
        let value = serde_json::to_value(data).unwrap_or(serde_json::Value::Null);
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── SseRoute ───────────────────────────────────────────
    #[test]
    fn sse_route_deref_returns_path() {
        const ROUTE: SseRoute = SseRoute::new("/events/feed");
        assert_eq!(&*ROUTE, "/events/feed");
        assert_eq!(ROUTE.path(), "/events/feed");
    }

    #[test]
    fn sse_route_as_ref_works() {
        const ROUTE: SseRoute = SseRoute::new("/events/live");
        let s: &str = ROUTE.as_ref();
        assert_eq!(s, "/events/live");
    }

    // ── SilcrowEvent::patch ────────────────────────────────
    #[test]
    fn patch_event_serializes_correct_wire_format() {
        let evt = SilcrowEvent::patch(
            serde_json::json!({"count": 42, "status": "online"}),
            "#dashboard",
        );
        let sse_event: Event = evt.into();

        // Convert to string to verify format
        let rendered = format!("{sse_event}");
        assert!(rendered.contains("event:patch"));
        assert!(rendered.contains("\"target\":\"#dashboard\""));
        assert!(rendered.contains("\"count\":42"));
        assert!(rendered.contains("\"status\":\"online\""));
    }

    #[test]
    fn patch_event_with_struct_data() {
        #[derive(serde::Serialize)]
        struct Stats {
            online: u32,
            active: bool,
        }

        let evt = SilcrowEvent::patch(
            Stats {
                online: 100,
                active: true,
            },
            "#stats",
        );
        let sse_event: Event = evt.into();
        let rendered = format!("{sse_event}");
        assert!(rendered.contains("event:patch"));
        assert!(rendered.contains("\"online\":100"));
        assert!(rendered.contains("\"active\":true"));
        assert!(rendered.contains("\"target\":\"#stats\""));
    }

    // ── SilcrowEvent::html ─────────────────────────────────
    #[test]
    fn html_event_serializes_correct_wire_format() {
        let evt = SilcrowEvent::html("<p>Updated</p>", "#content");
        let sse_event: Event = evt.into();

        let rendered = format!("{sse_event}");
        assert!(rendered.contains("event:html"));
        assert!(rendered.contains("\"target\":\"#content\""));
        assert!(rendered.contains("<p>Updated</p>"));
    }

    #[test]
    fn html_event_with_string_owned() {
        let markup = format!("<div>{}</div>", "dynamic");
        let evt = SilcrowEvent::html(markup, "#app");
        let sse_event: Event = evt.into();
        let rendered = format!("{sse_event}");
        assert!(rendered.contains("event:html"));
        assert!(rendered.contains("<div>dynamic</div>"));
    }

    // ── SilcrowEvent edge cases ────────────────────────────
    #[test]
    fn patch_event_with_empty_object() {
        let evt = SilcrowEvent::patch(serde_json::json!({}), "#empty");
        let sse_event: Event = evt.into();
        let rendered = format!("{sse_event}");
        assert!(rendered.contains("event:patch"));
        assert!(rendered.contains("\"target\":\"#empty\""));
        assert!(rendered.contains("\"data\":{}"));
    }

    #[test]
    fn patch_event_with_array_data() {
        let evt = SilcrowEvent::patch(serde_json::json!([{"key": "1", "name": "Alice"}]), "#list");
        let sse_event: Event = evt.into();
        let rendered = format!("{sse_event}");
        assert!(rendered.contains("event:patch"));
        assert!(rendered.contains("\"target\":\"#list\""));
        assert!(rendered.contains("\"name\":\"Alice\""));
    }

    #[test]
    fn html_event_with_empty_markup() {
        let evt = SilcrowEvent::html("", "#slot");
        let sse_event: Event = evt.into();
        let rendered = format!("{sse_event}");
        assert!(rendered.contains("event:html"));
        assert!(rendered.contains("\"target\":\"#slot\""));
        assert!(rendered.contains("\"html\":\"\""));
    }

    #[test]
    fn patch_event_with_nested_data() {
        let evt = SilcrowEvent::patch(
            serde_json::json!({"user": {"profile": {"name": "Bob"}}}),
            "#deep",
        );
        let sse_event: Event = evt.into();
        let rendered = format!("{sse_event}");
        assert!(rendered.contains("\"name\":\"Bob\""));
        assert!(rendered.contains("\"target\":\"#deep\""));
    }

    // ── sse() function ─────────────────────────────────────
    #[tokio::test]
    async fn sse_function_returns_event_stream_content_type() {
        use axum::response::IntoResponse;
        use futures_core::Stream;
        use std::pin::Pin;
        use std::task::{Context, Poll};

        // Minimal stream that yields one event then ends
        struct OneShot(bool);
        impl Stream for OneShot {
            type Item = Result<Event, Infallible>;
            fn poll_next(
                mut self: Pin<&mut Self>,
                _cx: &mut Context<'_>,
            ) -> Poll<Option<Self::Item>> {
                if self.0 {
                    Poll::Ready(None)
                } else {
                    self.0 = true;
                    let evt = SilcrowEvent::patch(serde_json::json!({"ok": true}), "#test");
                    Poll::Ready(Some(Ok(evt.into())))
                }
            }
        }

        let response = sse(OneShot(false)).into_response();
        let ct = response
            .headers()
            .get("content-type")
            .expect("should have content-type")
            .to_str()
            .expect("should be utf8");
        assert!(
            ct.contains("text/event-stream"),
            "expected text/event-stream, got: {ct}"
        );
    }

    // ── SseRoute with ResponseExt ──────────────────────────
    #[test]
    fn sse_route_works_with_response_ext_sse_method() {
        use crate::response::{html, ResponseExt};
        use axum::response::IntoResponse;

        const FEED: SseRoute = SseRoute::new("/events/feed");
        let response = html("<div id='feed'></div>")
            .sse(FEED.path())
            .into_response();

        assert_eq!(response.headers()["silcrow-sse"], "/events/feed");
    }

    #[test]
    fn sse_route_chains_with_other_modifiers() {
        use crate::response::{html, ResponseExt};
        use axum::response::IntoResponse;

        const LIVE: SseRoute = SseRoute::new("/events/dashboard");
        let response = html("<div id='dash'></div>")
            .sse(LIVE.path())
            .no_cache()
            .retarget("#main")
            .with_toast("Connected", "info")
            .into_response();

        assert_eq!(response.headers()["silcrow-sse"], "/events/dashboard");
        assert_eq!(response.headers()["silcrow-cache"], "no-cache");
        assert_eq!(response.headers()["silcrow-retarget"], "#main");

        let cookies: Vec<_> = response
            .headers()
            .get_all(axum::http::header::SET_COOKIE)
            .iter()
            .map(|v| v.to_str().unwrap().to_string())
            .collect();
        assert!(cookies.iter().any(|c| c.starts_with("silcrow_toasts=")));
    }

    #[test]
    fn sse_route_works_on_json_response() {
        use crate::response::{json, ResponseExt};
        use axum::response::IntoResponse;

        const STREAM: SseRoute = SseRoute::new("/events/updates");
        let response = json(serde_json::json!({"status": "ok"}))
            .sse(STREAM.path())
            .into_response();

        assert_eq!(response.headers()["silcrow-sse"], "/events/updates");
    }

    #[test]
    fn sse_route_works_on_navigate_response() {
        use crate::response::{navigate, ResponseExt};
        use axum::response::IntoResponse;

        const NOTIFY: SseRoute = SseRoute::new("/events/notify");
        let response = navigate("/dashboard").sse(NOTIFY.path()).into_response();

        assert_eq!(response.status(), axum::http::StatusCode::SEE_OTHER);
        assert_eq!(response.headers()["silcrow-sse"], "/events/notify");
    }
}
