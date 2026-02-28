# Server-Sent Events (SSE)

This guide walks through building a real-time dashboard that pushes updates to the browser using SSE.

## Overview

SSE is a one-way channel: server → client. Silcrow.js manages the `EventSource` connection, automatic reconnection, and dispatching events to the right DOM targets — you just define the stream.

## Step 1: Define a Route Constant

```rust
use pilcrow::SseRoute;

pub const DASH_EVENTS: SseRoute = SseRoute::new("/events/dashboard");
```

`SseRoute` is a typed newtype. Use the same constant for both the route registration and the `.sse()` modifier — this keeps paths in sync at compile time.

## Step 2: Page Handler

Tell Silcrow.js to open an SSE connection by adding `.sse()` to the response:

```rust
use pilcrow::*;

async fn dashboard(req: SilcrowRequest) -> Result<Response, Response> {
    let stats = db.get_stats().await?;
    let markup = render_dashboard(&stats);

    respond!(req, {
        html => html(layout("Dashboard", &markup)).sse(DASH_EVENTS),
        json => json(&stats),
    })
}
```

The `.sse()` modifier sets the `silcrow-sse` header. Silcrow.js reads it and opens an `EventSource` to `/events/dashboard` automatically.

## Step 3: SSE Stream Handler

Create the event stream using `async-stream`:

```rust
use pilcrow::{sse, SilcrowEvent};
use axum::response::IntoResponse;
use std::convert::Infallible;
use std::time::Duration;

async fn dashboard_stream() -> impl IntoResponse {
    let stream = async_stream::stream! {
        loop {
            let stats = db.get_stats().await;

            // Send JSON patch to #dashboard
            yield Ok::<_, Infallible>(
                SilcrowEvent::patch(&stats, "#dashboard").into()
            );

            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    };

    pilcrow::sse(stream)
}
```

## Step 4: Wire Up the Router

```rust
use axum::{routing::get, Router};

let app = Router::new()
    .route("/dashboard", get(dashboard))
    .route(DASH_EVENTS.path(), get(dashboard_stream));
```

## SilcrowEvent Types

### `SilcrowEvent::patch(data, target)`

Sends serializable data to `Silcrow.patch(data, target)` on the client. The target element's `s-bind` attributes are updated:

```rust
// Server
SilcrowEvent::patch(
    serde_json::json!({"visitors": 1234, "cpu": 45.2}),
    "#stats"
)
```

```html
<!-- Client: these get updated automatically -->
<div id="stats">
  <span s-bind="visitors">0</span>
  <span s-bind="cpu">0</span>
</div>
```

### `SilcrowEvent::html(markup, target)`

Sends raw HTML to be swapped into the target element:

```rust
SilcrowEvent::html(
    "<li>New log entry at 12:34</li>",
    "#log-feed"
)
```

Useful for appending to lists, updating complex structures, or replacing content that isn't easily expressed as a JSON patch.

## Using Templates in SSE Events

You can render Maud or Askama templates inside SSE events:

```rust
// With Maud
let markup = maud::html! {
    tr {
        td { (order.id) }
        td { (order.status) }
        td { "$" (order.total) }
    }
}
.into_string();

yield Ok::<_, Infallible>(
    SilcrowEvent::html(markup, "#orders-table tbody").into()
);
```

```rust
// With Askama
#[derive(askama::Template)]
#[template(path = "order_row.html")]
struct OrderRow<'a> {
    order: &'a Order,
}

let markup = OrderRow { order: &order }.render().unwrap();
yield Ok::<_, Infallible>(
    SilcrowEvent::html(markup, "#orders-table tbody").into()
);
```

## Client-Side Behavior

Silcrow.js handles everything automatically:

- **Connection:** opens `EventSource` when the `silcrow-sse` header is received
- **Reconnection:** exponential backoff (1s → 2s → 4s → max 30s) on disconnect
- **Event dispatch:** routes `patch` and `html` events to the correct DOM targets
- **Cleanup:** closes connection when the triggering element leaves the DOM

See the [Silcrow.js SSE docs](silcrow.md#live-sse-connections--real-time-updates) for advanced client-side configuration.

## Testing

```bash
# See the SSE stream directly
curl -N http://127.0.0.1:3000/events/dashboard

# Output:
# event: patch
# data: {"target":"#dashboard","data":{"visitors":1234,"cpu":45.2}}
#
# event: patch
# data: {"target":"#dashboard","data":{"visitors":1244,"cpu":45.5}}
```

## Next Steps

- [WebSocket](ws-guide.md) — bidirectional real-time communication
- [Response Modifiers](response-modifiers.md) — all `.sse()` and related methods
