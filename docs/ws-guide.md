# WebSocket

This guide walks through building a real-time chat feature with bidirectional WebSocket communication.

## Overview

WebSocket is a two-way channel: server ↔ client. Use it when the client needs to **send** data to the server in real-time (chat, collaborative editing, game state). For one-way server → client updates, prefer [SSE](sse-guide.md) — it's simpler.

## Step 1: Define a Route Constant

```rust
use pilcrow::WsRoute;

pub const CHAT_WS: WsRoute = WsRoute::new("/ws/chat");
```

## Step 2: Page Handler

Tell Silcrow.js to open a WebSocket connection:

```rust
use pilcrow::*;

async fn chat_page(req: SilcrowRequest) -> Result<Response, Response> {
    let history = db.recent_messages().await?;
    let markup = render_chat(&history);

    respond!(req, {
        html => html(layout("Chat", &markup)).ws(CHAT_WS),
        json => json(&history),
    })
}
```

## Step 3: WebSocket Handler

Use `ws::ws()` to upgrade the HTTP connection and handle it with a typed `WsStream`:

```rust
use axum::extract::ws::WebSocketUpgrade;
use axum::response::IntoResponse;
use pilcrow::ws::{ws, WsEvent, WsStream};

async fn chat_handler(upgrade: WebSocketUpgrade) -> impl IntoResponse {
    ws(upgrade, |mut stream| async move {
        // Send a welcome message
        stream
            .send(WsEvent::patch(
                serde_json::json!({"status": "connected"}),
                "#chat-status",
            ))
            .await
            .ok();

        // Process incoming messages
        while let Some(Ok(event)) = stream.recv().await {
            match event {
                WsEvent::Custom { event: name, data } => {
                    // Save to DB, broadcast, etc.
                    let saved = db.save_message(&data).await;
                    stream
                        .send(WsEvent::html(
                            render_message(&saved),
                            "#messages",
                        ))
                        .await
                        .ok();
                }
                _ => {}
            }
        }
    })
}
```

## Step 4: Wire Up the Router

```rust
let app = Router::new()
    .route("/chat", get(chat_page))
    .route(CHAT_WS.path(), get(chat_handler));
```

## WsEvent — All 5 Variants

### `WsEvent::patch(data, target)`

Send JSON data to be patched into a DOM element:

```rust
stream.send(WsEvent::patch(
    serde_json::json!({"online": 42}),
    "#user-count"
)).await.ok();
```

### `WsEvent::html(markup, target)`

Send HTML to be swapped into a DOM element:

```rust
stream.send(WsEvent::html(
    "<p><strong>Alice:</strong> Hello!</p>",
    "#messages"
)).await.ok();
```

### `WsEvent::invalidate(target)`

Tell the client to rebuild binding maps for a subtree:

```rust
stream.send(WsEvent::invalidate("#user-list")).await.ok();
```

### `WsEvent::navigate(path)`

Tell the client to perform a navigation:

```rust
stream.send(WsEvent::navigate("/logout")).await.ok();
```

### `WsEvent::custom(event_name, data)`

Application-defined event with arbitrary data:

```rust
stream.send(WsEvent::custom(
    "notification",
    serde_json::json!({"title": "New message", "from": "Alice"})
)).await.ok();
```

## Client → Server: Sending Data

On the client side, use `Silcrow.send()`:

```html
<div id="chat" s-live="/ws/chat">
  <input id="msg" type="text" />
  <button onclick="Silcrow.send(
    document.getElementById('chat'),
    {event: 'chat', data: {text: document.getElementById('msg').value}}
  )">Send</button>
</div>
```

The server receives this as `WsEvent::Custom { event: "chat", data: {...} }`.

## Using Templates in WS Events

Render Maud or Askama templates inside WebSocket responses:

```rust
// With Maud
let markup = maud::html! {
    div.message {
        strong { (msg.author) ": " }
        span { (msg.text) }
        time { (msg.timestamp) }
    }
}
.into_string();

stream.send(WsEvent::html(markup, "#messages")).await.ok();
```

## WsStream Methods

| Method | Description |
| --- | --- |
| `stream.send(event)` | Send a `WsEvent` as JSON text frame |
| `stream.recv()` | Receive next `WsEvent` (returns `Option<Result>`) |
| `stream.close()` | Send close frame and consume the stream |

`recv()` returns:

- `None` — connection fully closed
- `Some(Ok(event))` — valid `WsEvent`
- `Some(Err(WsRecvError::Closed))` — close frame received
- `Some(Err(WsRecvError::NonText))` — binary message (unsupported)
- `Some(Err(WsRecvError::Deserialize(_)))` — invalid JSON

## Wire Format

All WsEvents serialize as tagged JSON:

```json
{"type": "patch", "target": "#stats", "data": {"count": 42}}
{"type": "html", "target": "#content", "markup": "<p>Hello</p>"}
{"type": "invalidate", "target": "#form"}
{"type": "navigate", "path": "/dashboard"}
{"type": "custom", "event": "ping", "data": {"ts": 12345}}
```

## Client-Side Behavior

Silcrow.js handles:

- **Connection:** opens WebSocket when `silcrow-ws` header is received
- **Reconnection:** exponential backoff on disconnect
- **Message dispatch:** routes events to correct handlers based on `type`
- **Cleanup:** closes connection when the `s-live` element leaves the DOM

See the [Silcrow.js WebSocket docs](silcrow.md#websocket-with-s-live) for the full client-side API.

## Next Steps

- [Response Modifiers](response-modifiers.md) — all `.ws()` and related methods
