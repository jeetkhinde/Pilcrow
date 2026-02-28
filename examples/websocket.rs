// examples/websocket.rs
//
// WebSocket — bidirectional real-time communication.
//
// Run:  cargo run --example websocket
// Test:
//   curl http://127.0.0.1:3000              (HTML page with WS header)
//   websocat ws://127.0.0.1:3000/ws/chat    (raw WS connection)

use axum::{
    extract::ws::WebSocketUpgrade,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use pilcrow::{ws::WsEvent, *};
use serde::Serialize;

// ── Route constant ──────────────────────────────────────────

const CHAT_WS: WsRoute = WsRoute::new("/ws/chat");

// ── Data ────────────────────────────────────────────────────

#[derive(Serialize)]
struct ChatMessage {
    user: String,
    text: String,
    timestamp: u64,
}

// ── Handlers ────────────────────────────────────────────────

/// Page handler — tells Silcrow.js to open a WebSocket connection
async fn chat_page(req: SilcrowRequest) -> Result<Response, Response> {
    respond!(req, {
        html => html(r#"<html>
<body s-debug>
  <h1>Chat</h1>
  <div id="chat-status" s-bind="status">Connecting...</div>
  <div id="messages"></div>
</body>
</html>"#).ws(CHAT_WS),
        json => json(serde_json::json!({"ws": CHAT_WS.path()})),
    })
}

/// WebSocket handler — echo server demonstrating all WsEvent variants
async fn chat_ws(upgrade: WebSocketUpgrade) -> impl IntoResponse {
    ws::ws(upgrade, |mut stream| async move {
        // Send a welcome patch
        stream
            .send(WsEvent::patch(
                serde_json::json!({"status": "connected"}),
                "#chat-status",
            ))
            .await
            .ok();

        // Send an HTML greeting
        stream
            .send(WsEvent::html(
                "<p><strong>System:</strong> Welcome to the chat!</p>",
                "#messages",
            ))
            .await
            .ok();

        // Process incoming messages
        while let Some(Ok(event)) = stream.recv().await {
            match event {
                // Echo custom events as patch updates
                WsEvent::Custom { event: name, data } => {
                    let response_msg = ChatMessage {
                        user: "Echo".into(),
                        text: format!("You sent event '{}': {}", name, data),
                        timestamp: 0,
                    };
                    stream
                        .send(WsEvent::patch(response_msg, "#messages"))
                        .await
                        .ok();
                }

                // Echo patch events back
                WsEvent::Patch { data, target } => {
                    stream
                        .send(WsEvent::html(
                            format!("<p>Echoed patch to {}: {}</p>", target, data),
                            "#messages",
                        ))
                        .await
                        .ok();
                }

                // Demonstrate invalidate
                WsEvent::Invalidate { target } => {
                    stream.send(WsEvent::invalidate(&target)).await.ok();
                }

                // Demonstrate navigate
                WsEvent::Navigate { path } => {
                    stream.send(WsEvent::navigate(&path)).await.ok();
                }

                _ => {}
            }
        }

        // Connection closed — graceful cleanup happens automatically
    })
}

// ── Main ────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(chat_page))
        .route(CHAT_WS.path(), get(chat_ws));

    println!("Listening on http://127.0.0.1:3000");
    println!("  GET /         — chat page (sets silcrow-ws header)");
    println!("  WS  /ws/chat  — WebSocket endpoint");

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}
