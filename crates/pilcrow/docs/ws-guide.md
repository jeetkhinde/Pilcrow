# WebSocket

WebSocket in Pilcrow convention:

- web owns browser socket endpoint
- backend remains source of domain truth
- web forwards/persists through backend APIs/services

## Route Constant

```rust
pub const CHAT_WS: WsRoute = WsRoute::new("/ws/chat");
```

## Page Handler

```rust
async fn chat_page(State(state): State<AppState>) -> Response {
    let history = match state.chat_api.recent_messages().await {
        Ok(history) => history,
        Err(err) => return (StatusCode::BAD_GATEWAY, Html(err.to_string())).into_response(),
    };

    let html = render_chat(&history);
    pilcrow_web::html(html).ws(CHAT_WS).into_response()
}
```

## Socket Handler

```rust
async fn chat_handler(upgrade: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    pilcrow_web::ws::ws(upgrade, move |mut stream| {
        let state = state.clone();
        async move {
            while let Some(Ok(event)) = stream.recv().await {
                if let WsEvent::Custom { event: _, data } = event {
                    if let Ok(saved) = state.chat_api.save_message(data).await {
                        let _ = stream.send(WsEvent::html(render_message(&saved), "#messages")).await;
                    }
                }
            }
        }
    })
}
```
