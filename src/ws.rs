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
