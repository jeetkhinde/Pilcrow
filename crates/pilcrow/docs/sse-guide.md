# Server-Sent Events (SSE)

SSE in Pilcrow convention:

- web page enables SSE
- web stream handler pushes updates
- stream data comes from backend APIs/services

## Route Constant

```rust
pub const DASH_EVENTS: SseRoute = SseRoute::new("/events/dashboard");
```

## Page Handler

```rust
async fn dashboard(State(state): State<AppState>) -> Response {
    let stats = match state.dashboard_api.snapshot().await {
        Ok(stats) => stats,
        Err(err) => return (StatusCode::BAD_GATEWAY, Html(err.to_string())).into_response(),
    };

    let html = render_dashboard_page(&stats);
    pilcrow_web::html(html).sse(DASH_EVENTS).into_response()
}
```

## Stream Handler

```rust
async fn dashboard_stream(State(state): State<AppState>) -> impl IntoResponse {
    let stream = async_stream::stream! {
        loop {
            if let Ok(stats) = state.dashboard_api.snapshot().await {
                yield Ok::<_, Infallible>(SilcrowEvent::patch(stats, "#dashboard").into());
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    };

    pilcrow_web::sse(stream)
}
```
