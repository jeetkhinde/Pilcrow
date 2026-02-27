// ./src/ws.rs

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::{IntoResponse, Response};
use std::future::Future;
use std::ops::Deref;

// ════════════════════════════════════════════════════════════
// 1. WsRoute — typed route constant for WS endpoints
// ════════════════════════════════════════════════════════════

/// A compile-time WebSocket route path. Use as both a route string and header value.
///
/// ```ignore
/// const CHAT: WsRoute = WsRoute::new("/ws/chat");
/// // Route:  .route(CHAT.path(), get(chat_handler))
/// // Header: html(markup).ws(CHAT)
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WsRoute(&'static str);

impl WsRoute {
    pub const fn new(path: &'static str) -> Self {
        Self(path)
    }

    pub const fn path(&self) -> &'static str {
        self.0
    }
}

impl Deref for WsRoute {
    type Target = str;
    fn deref(&self) -> &str {
        self.0
    }
}

impl AsRef<str> for WsRoute {
    fn as_ref(&self) -> &str {
        self.0
    }
}

// ════════════════════════════════════════════════════════════
// 2. WsEvent — bidirectional message enum
// ════════════════════════════════════════════════════════════

/// A structured WebSocket message that both Rust and Silcrow.js understand.
///
/// Serializes as JSON with a `type` tag for dispatch:
/// ```json
/// {"type": "patch", "target": "#stats", "data": {"count": 42}}
/// ```
///
/// Five variants cover the full Silcrow instruction set:
/// - `Patch` — send JSON data to be patched into a target element
/// - `Html` — send HTML markup to be swapped into a target element
/// - `Invalidate` — tell client to drop binding cache for a target
/// - `Navigate` — tell client to navigate to a path
/// - `Custom` — application-defined event with arbitrary data
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsEvent {
    Patch {
        target: String,
        data: serde_json::Value,
    },
    Html {
        target: String,
        markup: String,
    },
    Invalidate {
        target: String,
    },
    Navigate {
        path: String,
    },
    Custom {
        event: String,
        data: serde_json::Value,
    },
}

impl WsEvent {
    /// Create a patch event that sends JSON data to a target element.
    ///
    /// ```ignore
    /// let evt = WsEvent::patch(json!({"count": 42}), "#stats");
    /// stream.send(evt).await?;
    /// ```
    pub fn patch(data: impl serde::Serialize, target: &str) -> Self {
        let value = serde_json::to_value(data).unwrap_or(serde_json::Value::Null);
        Self::Patch {
            target: target.to_owned(),
            data: value,
        }
    }

    /// Create an HTML event that sends markup to a target element.
    ///
    /// ```ignore
    /// let evt = WsEvent::html("<p>Updated</p>", "#content");
    /// stream.send(evt).await?;
    /// ```
    pub fn html(markup: impl Into<String>, target: &str) -> Self {
        Self::Html {
            target: target.to_owned(),
            markup: markup.into(),
        }
    }

    /// Create an invalidate event that drops binding cache for a target.
    ///
    /// ```ignore
    /// let evt = WsEvent::invalidate("#user-card");
    /// stream.send(evt).await?;
    /// ```
    pub fn invalidate(target: &str) -> Self {
        Self::Invalidate {
            target: target.to_owned(),
        }
    }

    /// Create a navigate event that tells the client to go to a path.
    ///
    /// ```ignore
    /// let evt = WsEvent::navigate("/dashboard");
    /// stream.send(evt).await?;
    /// ```
    pub fn navigate(path: impl Into<String>) -> Self {
        Self::Navigate { path: path.into() }
    }

    /// Create a custom event with application-defined name and data.
    ///
    /// ```ignore
    /// let evt = WsEvent::custom("refresh", json!({"section": "sidebar"}));
    /// stream.send(evt).await?;
    /// ```
    pub fn custom(event: impl Into<String>, data: impl serde::Serialize) -> Self {
        let value = serde_json::to_value(data).unwrap_or(serde_json::Value::Null);
        Self::Custom {
            event: event.into(),
            data: value,
        }
    }
}

// ════════════════════════════════════════════════════════════
// 3. WsRecvError — typed receive errors
// ════════════════════════════════════════════════════════════

/// Errors that can occur when receiving a `WsEvent` from a WebSocket.
#[derive(Debug)]
pub enum WsRecvError {
    /// The received message was valid text but not valid WsEvent JSON.
    Deserialize(serde_json::Error),
    /// The connection was closed (received Close frame).
    Closed,
    /// The received message was binary or ping/pong, not text.
    NonText,
}

impl std::fmt::Display for WsRecvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Deserialize(e) => write!(f, "WsRecvError::Deserialize: {e}"),
            Self::Closed => write!(f, "WsRecvError::Closed"),
            Self::NonText => write!(f, "WsRecvError::NonText"),
        }
    }
}

impl std::error::Error for WsRecvError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Deserialize(e) => Some(e),
            _ => None,
        }
    }
}

// ════════════════════════════════════════════════════════════
// 4. WsStream — typed wrapper around Axum's WebSocket
// ════════════════════════════════════════════════════════════

/// A typed WebSocket connection that sends and receives `WsEvent` messages.
///
/// Wraps Axum's `WebSocket` to provide JSON serialization/deserialization
/// of Silcrow-compatible events.
///
/// ```ignore
/// async fn chat_handler(upgrade: WebSocketUpgrade) -> Response {
///     pilcrow::ws(upgrade, |mut stream| async move {
///         while let Some(Ok(event)) = stream.recv().await {
///             match event {
///                 WsEvent::Custom { event, data } => {
///                     stream.send(WsEvent::patch(data, "#chat")).await.ok();
///                 }
///                 _ => {}
///             }
///         }
///     })
/// }
/// ```
#[derive(Debug)]
pub struct WsStream {
    socket: WebSocket,
}

impl WsStream {
    /// Wrap an Axum WebSocket in a typed Silcrow stream.
    pub fn new(socket: WebSocket) -> Self {
        Self { socket }
    }

    /// Send a `WsEvent` as a JSON text frame.
    ///
    /// Returns `Err` if serialization fails or the connection is broken.
    pub async fn send(&mut self, event: WsEvent) -> Result<(), axum::Error> {
        match serde_json::to_string(&event) {
            Ok(json) => self.socket.send(Message::Text(json)).await,
            Err(e) => {
                tracing::warn!("WsStream::send serialization failed: {e}");
                Err(axum::Error::new(e))
            }
        }
    }

    /// Receive the next `WsEvent` from the connection.
    ///
    /// Returns `None` when the connection is fully closed.
    /// Returns `Some(Err(...))` for close frames, binary messages, or bad JSON.
    pub async fn recv(&mut self) -> Option<Result<WsEvent, WsRecvError>> {
        loop {
            match self.socket.recv().await {
                None => return None,
                Some(Err(_)) => return None,
                Some(Ok(msg)) => match msg {
                    Message::Text(text) => {
                        return Some(serde_json::from_str(&text).map_err(WsRecvError::Deserialize));
                    }
                    Message::Close(_) => return Some(Err(WsRecvError::Closed)),
                    Message::Ping(_) | Message::Pong(_) => continue,
                    Message::Binary(_) => return Some(Err(WsRecvError::NonText)),
                },
            }
        }
    }

    /// Gracefully close the WebSocket connection.
    pub async fn close(mut self) {
        let _ = self.socket.send(Message::Close(None)).await;
    }
}

// ════════════════════════════════════════════════════════════
// 5. ws() — upgrade helper
// ════════════════════════════════════════════════════════════

/// Upgrade an HTTP connection to a WebSocket and hand it to a typed handler.
///
/// ```ignore
/// async fn handler(upgrade: WebSocketUpgrade) -> Response {
///     pilcrow::ws(upgrade, |mut stream| async move {
///         stream.send(WsEvent::patch(json!({"ready": true}), "#app")).await.ok();
///         while let Some(Ok(event)) = stream.recv().await {
///             // handle events
///         }
///     })
/// }
/// ```
pub fn ws<F, Fut>(upgrade: WebSocketUpgrade, handler: F) -> Response
where
    F: FnOnce(WsStream) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    upgrade
        .on_upgrade(|socket| async move {
            handler(WsStream::new(socket)).await;
        })
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── WsRoute ────────────────────────────────────────────

    #[test]
    fn ws_route_deref_returns_path() {
        const ROUTE: WsRoute = WsRoute::new("/ws/chat");
        assert_eq!(&*ROUTE, "/ws/chat");
        assert_eq!(ROUTE.path(), "/ws/chat");
    }

    #[test]
    fn ws_route_as_ref_works() {
        const ROUTE: WsRoute = WsRoute::new("/ws/live");
        let s: &str = ROUTE.as_ref();
        assert_eq!(s, "/ws/live");
    }

    #[test]
    fn ws_route_equality() {
        const A: WsRoute = WsRoute::new("/ws/feed");
        const B: WsRoute = WsRoute::new("/ws/feed");
        assert_eq!(A, B);
    }

    // ── WsEvent serde round-trip ───────────────────────────

    #[test]
    fn patch_event_round_trip() {
        let evt = WsEvent::patch(serde_json::json!({"count": 42}), "#stats");
        let json = serde_json::to_string(&evt).expect("serialize");
        let parsed: WsEvent = serde_json::from_str(&json).expect("deserialize");
        match parsed {
            WsEvent::Patch { target, data } => {
                assert_eq!(target, "#stats");
                assert_eq!(data["count"], 42);
            }
            other => panic!("expected Patch, got {other:?}"),
        }
    }

    #[test]
    fn html_event_round_trip() {
        let evt = WsEvent::html("<p>Hello</p>", "#content");
        let json = serde_json::to_string(&evt).expect("serialize");
        let parsed: WsEvent = serde_json::from_str(&json).expect("deserialize");
        match parsed {
            WsEvent::Html { target, markup } => {
                assert_eq!(target, "#content");
                assert_eq!(markup, "<p>Hello</p>");
            }
            other => panic!("expected Html, got {other:?}"),
        }
    }

    #[test]
    fn invalidate_event_round_trip() {
        let evt = WsEvent::invalidate("#card");
        let json = serde_json::to_string(&evt).expect("serialize");
        let parsed: WsEvent = serde_json::from_str(&json).expect("deserialize");
        match parsed {
            WsEvent::Invalidate { target } => assert_eq!(target, "#card"),
            other => panic!("expected Invalidate, got {other:?}"),
        }
    }

    #[test]
    fn navigate_event_round_trip() {
        let evt = WsEvent::navigate("/dashboard");
        let json = serde_json::to_string(&evt).expect("serialize");
        let parsed: WsEvent = serde_json::from_str(&json).expect("deserialize");
        match parsed {
            WsEvent::Navigate { path } => assert_eq!(path, "/dashboard"),
            other => panic!("expected Navigate, got {other:?}"),
        }
    }

    #[test]
    fn custom_event_round_trip() {
        let evt = WsEvent::custom("refresh", serde_json::json!({"section": "sidebar"}));
        let json = serde_json::to_string(&evt).expect("serialize");
        let parsed: WsEvent = serde_json::from_str(&json).expect("deserialize");
        match parsed {
            WsEvent::Custom { event, data } => {
                assert_eq!(event, "refresh");
                assert_eq!(data["section"], "sidebar");
            }
            other => panic!("expected Custom, got {other:?}"),
        }
    }

    #[test]
    fn patch_event_wire_format_has_type_tag() {
        let evt = WsEvent::patch(serde_json::json!({"ok": true}), "#el");
        let json = serde_json::to_string(&evt).expect("serialize");
        assert!(json.contains("\"type\":\"patch\""));
        assert!(json.contains("\"target\":\"#el\""));
    }

    #[test]
    fn patch_event_with_struct_data() {
        #[derive(serde::Serialize)]
        struct Stats {
            online: u32,
            active: bool,
        }
        let evt = WsEvent::patch(
            Stats {
                online: 5,
                active: true,
            },
            "#stats",
        );
        let json = serde_json::to_string(&evt).expect("serialize");
        assert!(json.contains("\"online\":5"));
        assert!(json.contains("\"active\":true"));
    }

    #[test]
    fn patch_event_with_unserializable_falls_back_to_null() {
        // f64::NAN is not valid JSON — should fall back to Value::Null
        let evt = WsEvent::patch(f64::NAN, "#el");
        match evt {
            WsEvent::Patch { data, .. } => assert!(data.is_null()),
            _ => panic!("expected Patch"),
        }
    }

    // ── WsRecvError Display ────────────────────────────────

    #[test]
    fn recv_error_display() {
        let closed = WsRecvError::Closed;
        assert_eq!(format!("{closed}"), "WsRecvError::Closed");

        let non_text = WsRecvError::NonText;
        assert_eq!(format!("{non_text}"), "WsRecvError::NonText");
    }

    // ── .ws() ResponseExt header ───────────────────────────

    #[test]
    fn ws_header_on_html_response() {
        use crate::response::{html, ResponseExt};
        use axum::response::IntoResponse;

        const CHAT: WsRoute = WsRoute::new("/ws/chat");
        let response = html("<div id='chat'></div>")
            .ws(CHAT.path())
            .into_response();

        assert_eq!(response.headers()["silcrow-ws"], "/ws/chat");
    }

    #[test]
    fn ws_header_on_json_response() {
        use crate::response::{json, ResponseExt};
        use axum::response::IntoResponse;

        const FEED: WsRoute = WsRoute::new("/ws/feed");
        let response = json(serde_json::json!({"status": "ok"}))
            .ws(FEED.path())
            .into_response();

        assert_eq!(response.headers()["silcrow-ws"], "/ws/feed");
    }

    #[test]
    fn ws_header_on_navigate_response() {
        use crate::response::{navigate, ResponseExt};
        use axum::response::IntoResponse;

        const NOTIFY: WsRoute = WsRoute::new("/ws/notify");
        let response = navigate("/dashboard").ws(NOTIFY.path()).into_response();

        assert_eq!(response.status(), axum::http::StatusCode::SEE_OTHER);
        assert_eq!(response.headers()["silcrow-ws"], "/ws/notify");
    }

    #[test]
    fn ws_chains_with_other_modifiers() {
        use crate::response::{html, ResponseExt};
        use axum::response::IntoResponse;

        const LIVE: WsRoute = WsRoute::new("/ws/dashboard");
        let response = html("<div id='dash'></div>")
            .ws(LIVE.path())
            .no_cache()
            .retarget("#main")
            .with_toast("Connected", "info")
            .into_response();

        assert_eq!(response.headers()["silcrow-ws"], "/ws/dashboard");
        assert_eq!(response.headers()["silcrow-cache"], "no-cache");
        assert_eq!(response.headers()["silcrow-retarget"], "#main");
    }

    #[test]
    fn ws_and_sse_coexist_on_same_response() {
        use crate::response::{html, ResponseExt};
        use crate::sse::SseRoute;
        use axum::response::IntoResponse;

        const WS_PATH: WsRoute = WsRoute::new("/ws/live");
        const SSE_PATH: SseRoute = SseRoute::new("/events/live");

        let response = html("<div id='live'></div>")
            .ws(WS_PATH.path())
            .sse(SSE_PATH.path())
            .into_response();

        assert_eq!(response.headers()["silcrow-ws"], "/ws/live");
        assert_eq!(response.headers()["silcrow-sse"], "/events/live");
    }
}
