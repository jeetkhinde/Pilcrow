# Pilcrow

A response layer for [Axum](https://github.com/tokio-rs/axum) that turns handlers into multi-modal engines — one handler serves HTML to browsers and JSON to API clients via content negotiation, lazy evaluation, and server-side orchestration of the [Silcrow.js](docs/silcrow.md) frontend runtime.

## Quick Start

```rust
use pilcrow::*;
use serde::Serialize;

#[derive(Serialize)]
struct Greeting {
    msg: String,
}

async fn home(req: SilcrowRequest) -> Result<Response, StatusCode> {
    pilcrow::respond!(req, {
        html => html("<h1>Hello</h1>"),
        json => json(Greeting { msg: "Hello".into() }),
    })
}

#[tokio::main]
async fn main() {
    use axum::{routing::get, Router};
    use pilcrow::assets::{serve_silcrow_js, silcrow_js_path};

    let app = Router::new()
        .route(&silcrow_js_path(), get(serve_silcrow_js))
        .route("/", get(home));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

## Table of Contents

- [Quick Start](#quick-start)
- [Dependencies](#dependencies)
- [Why Pilcrow?](#why-pilcrow)
- [Core Concepts](#core-concepts)
  - [1. The Extractor: `SilcrowRequest`](#1-the-extractor-silcrowrequest)
  - [2. The `respond!` Macro](#2-the-respond-macro)
  - [3. Modifiers via `ResponseExt`](#3-modifiers-via-responseext)
  - [Server-Driven Side Effects](#server-driven-side-effects)
  - [4. Navigation (Redirects)](#4-navigation-redirects)
  - [5. Server-Sent Events (SSE)](#5-server-sent-events-sse)
  - [6. WebSocket](#6-websocket)
- [Asset Serving](#asset-serving)
- [Examples](#examples)
- [Public API](#public-api)
- [License](#license)

## Dependencies

```toml
[dependencies]
pilcrow = "0.1"
```

Pilcrow depends on `axum 0.7`, `serde`, `serde_json`, `cookie`, `urlencoding`, `futures-core`, and `tracing`. No runtime overhead beyond what Axum already requires.

## Why Pilcrow?

Raw Axum requires manual header parsing and format switching in every handler:

```rust
async fn handler(headers: HeaderMap) -> Response {
    let accept = headers.get("Accept").and_then(|v| v.to_str().ok()).unwrap_or("");
    if accept.contains("text/html") {
        Html("<h1>Hi</h1>").into_response()
    } else {
        Json(user).into_response()
    }
}
```

Pilcrow replaces this with a declarative API:

```rust
async fn handler(req: SilcrowRequest) -> Result<Response, StatusCode> {
    pilcrow::respond!(req, {
        html => html("<h1>Hi</h1>"),
        json => json(user),
    })
}
```

Content negotiation, lazy evaluation, and response packaging are handled for you by Pilcrow.

## Core Concepts

### 1. The Extractor: `SilcrowRequest`

An Axum extractor that reads the `Accept` and `silcrow-target` headers to determine what the client wants. Use it as a handler argument — Axum injects it automatically.

```rust
pub async fn handler(req: SilcrowRequest) -> Result<Response, AppError> {
    // req.preferred_mode() returns RequestMode::Html or RequestMode::Json
}
```

The negotiation logic: Silcrow.js requests respect `Accept` strictly. Standard browser requests default to HTML. Everything else falls back to JSON.

### 2. The `respond!` Macro

The primary API. Declare your response arms and Pilcrow handles closure wrapping, async, and dispatch:

```rust
pilcrow::respond!(req, {
    html => html(markup),
    json => json(user),
})
```

**JSON shorthand with `raw`:** For serializable values where you don't need modifiers, skip the `json()` constructor:

```rust
pilcrow::respond!(req, {
    html => html(markup),
    json => raw user,        // auto-wrapped in json()
})
```

`raw` is JSON-only. HTML always uses the explicit `html()` constructor in Pilcrow.

**Shared toasts:** When both arms need the same toast, declare it once:

```rust
pilcrow::respond!(req, {
    html => html(markup),
    json => json(user),
    toast => ("Saved!", "success"),
})
```

The toast is applied to whichever branch runs.

**Single-arm handlers:** If an endpoint only serves one format, omit the other. Pilcrow returns `406 Not Acceptable` for unregistered formats automatically:

```rust
pilcrow::respond!(req, {
    json => json(status),
})
```

**Per-arm modifiers:** Chain modifiers directly on each arm:

```rust
pilcrow::respond!(req, {
    html => html(markup).with_toast("Updated", "success").no_cache(),
    json => json(user),
})
```

**All macro variants:**

```rust
// Both arms
pilcrow::respond!(req, { html => expr, json => expr })

// Both arms with raw JSON
pilcrow::respond!(req, { html => expr, json => raw expr })

// Both arms with shared toast
pilcrow::respond!(req, { html => expr, json => expr, toast => (msg, level) })

// HTML-only / JSON-only
pilcrow::respond!(req, { html => expr })
pilcrow::respond!(req, { json => expr })
pilcrow::respond!(req, { json => raw expr })

// Single arm with shared toast
pilcrow::respond!(req, { html => expr, toast => (msg, level) })
pilcrow::respond!(req, { json => expr, toast => (msg, level) })
```

### 3. Modifiers via `ResponseExt`

All response types (`HtmlResponse`, `JsonResponse`, `NavigateResponse`) implement `ResponseExt`, giving you a unified modifier chain:

| Method | Effect |
| --- | --- |
| `.with_toast(msg, level)` | Cookie-based for HTML/Navigate, payload-injected for JSON |
| `.with_header(key, value)` | Arbitrary response header |
| `.no_cache()` | Sets `silcrow-cache: no-cache` to prevent client caching |
| `.retarget(selector)` | Silcrow.js swaps HTML into a different DOM element |
| `.trigger_event(name)` | Fires a `CustomEvent` in the browser via Silcrow.js |
| `.push_history(url)` | Updates the browser URL bar without a page load |
| `.patch_target(selector, &data)` | Patches JSON data into a secondary DOM element via Silcrow.js |
| `.invalidate_target(selector)` | Rebuilds Silcrow.js binding maps for the target element |
| `.client_navigate(path)` | Triggers a client-side navigation via Silcrow.js |
| `.sse(route)` | Signals the client to open an SSE connection to the given path |
| `.ws(route)` | Signals the client to open a WebSocket connection to the given path |

Toast transport is automatic — HTML responses use a short-lived cookie (`Max-Age=5`, `SameSite=Lax`), JSON responses inject a `_toasts` array into the payload. If the JSON root isn't an object (e.g. you returned a `Vec`), Pilcrow wraps it as `{"data": [...], "_toasts": [...]}`.

### Server-Driven Side Effects

The last four modifiers above are server-driven — the response header tells Silcrow.js to perform an action after the main swap completes. This lets you orchestrate complex UI updates from a single response:

```rust
pub async fn save_item(req: SilcrowRequest) -> Result<Response, AppError> {
    let item = db.save_item(&payload).await?;
    let count = db.item_count().await?;

    pilcrow::respond!(req, {
        html => html(render_item(&item))
            .patch_target("#item-count", &serde_json::json!({"count": count}))
            .invalidate_target("#sidebar")
            .with_toast("Saved", "success"),
        json => json(item),
    })
}
```

Side effects execute in order: patch → invalidate → navigate → sse/ws. This lets a single response update the primary target, patch a secondary counter, rebuild a sidebar, and trigger a follow-up navigation.

### 4. Navigation (Redirects)

Redirects are imperative — they're not negotiated. Use `navigate()` for early returns like auth guards:

```rust
pub async fn admin(req: SilcrowRequest) -> Result<Response, AppError> {
    if !user.is_admin() {
        return Ok(navigate("/login")
            .with_toast("Unauthorized", "error")
            .into_response());
    }

    pilcrow::respond!(req, {
        html => html(admin_markup),
    })
}
```

`navigate()` returns a `303 See Other` with the `Location` header. Toasts persist across the redirect via cookie.

### 5. Server-Sent Events (SSE)

Pilcrow provides typed SSE support for real-time updates driven by Silcrow.js.

**Define a route constant:**

```rust
use pilcrow::SseRoute;

pub const DASHBOARD_EVENTS: SseRoute = SseRoute::new("/events/dashboard");
```

`SseRoute` is a typed newtype — use it in both the route registration and the `.sse()` modifier to keep paths in sync.

**Tell the client to connect:**

```rust
async fn dashboard(req: SilcrowRequest) -> Result<Response, AppError> {
    pilcrow::respond!(req, {
        html => html(markup).sse(DASHBOARD_EVENTS),
        json => json(data),
    })
}
```

The `.sse()` modifier sets the `silcrow-sse` header. Silcrow.js reads it and opens an `EventSource` connection automatically.

**Create the SSE endpoint:**

```rust
use pilcrow::{sse, SilcrowEvent};

async fn dashboard_events() -> impl IntoResponse {
    let stream = async_stream::stream! {
        loop {
            let stats = db.get_stats().await;
            yield Ok::<_, std::convert::Infallible>(
                SilcrowEvent::patch(stats, "#dashboard").into()
            );
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    };
    pilcrow::sse(stream)
}
```

`SilcrowEvent` has two constructors:

- `SilcrowEvent::patch(data, target)` — sends JSON data to `Silcrow.patch(data, target)`
- `SilcrowEvent::html(markup, target)` — sends HTML to `safeSetHTML(element, markup)`

Both serialize to the SSE wire format with named events that Silcrow.js understands.

**Register the route:**

```rust
Router::new()
    .route("/dashboard", get(dashboard))
    .route(DASHBOARD_EVENTS.path(), get(dashboard_events))
```

**Client-side:** Silcrow.js handles everything — connection management, reconnection with exponential backoff (1s → 2s → 4s → max 30s), and piping events to the right DOM targets. See the [Silcrow.js docs](docs/silcrow.md#live-sse-connections--real-time-updates) for the full client-side API.

---

### 6. WebSocket

Pilcrow provides typed WebSocket support for bidirectional real-time communication.

**Define a route constant:**

```rust
use pilcrow::WsRoute;

pub const CHAT_WS: WsRoute = WsRoute::new("/ws/chat");
```

**Tell the client to connect:**

```rust
async fn chat_page(req: SilcrowRequest) -> Result<Response, AppError> {
    pilcrow::respond!(req, {
        html => html(markup).ws(CHAT_WS),
        json => json(data),
    })
}
```

**Create the WebSocket endpoint:**

```rust
use axum::extract::ws::WebSocketUpgrade;
use pilcrow::ws::{ws, WsEvent, WsStream};

async fn chat_ws(upgrade: WebSocketUpgrade) -> Response {
    pilcrow::ws::ws(upgrade, |mut stream| async move {
        // Send a welcome message
        stream.send(WsEvent::patch(
            serde_json::json!({"status": "connected"}), "#chat-status"
        )).await.ok();

        // Echo incoming messages
        while let Some(Ok(event)) = stream.recv().await {
            if let WsEvent::Custom { event: name, data } = event {
                stream.send(WsEvent::patch(data, "#chat")).await.ok();
            }
        }
    })
}
```

`WsEvent` has five variants: `patch`, `html`, `invalidate`, `navigate`, and `custom`. All serialize as tagged JSON that Silcrow.js dispatches automatically.

**Register the route:**

```rust
Router::new()
    .route("/chat", get(chat_page))
    .route(CHAT_WS.path(), get(chat_ws))
```

**Client-side:** Silcrow.js handles connection management, reconnection with exponential backoff, and bidirectional messaging. Use `Silcrow.send(root, data)` to send messages from the client. See the [Silcrow.js docs](docs/silcrow.md#websocket-with-s-live) for the full client-side API.

---

## Asset Serving

Pilcrow embeds `silcrow.js` at compile time and serves it with a content-hashed URL for immutable caching. The hash is computed by `build.rs` at build time.

```rust
use pilcrow::assets::{serve_silcrow_js, silcrow_js_path, script_tag};

// Route the fingerprinted path
let app = Router::new()
    .route(&silcrow_js_path(), get(serve_silcrow_js));

// In your layout template — returns `<script src="/_silcrow/silcrow.{hash}.js" defer></script>`
fn layout(content: &str) -> String {
    format!("<html><head>{}</head><body>{content}</body></html>", script_tag())
}
```

The served response includes `Cache-Control: public, max-age=31536000, immutable`.

## Examples

### Dual-Mode Handler with DB

```rust
#[derive(Serialize)]
struct UserProfile {
    id: i64,
    name: String,
    bio: String,
}

pub async fn get_profile(
    req: SilcrowRequest,
    State(db): State<DbPool>,
) -> Result<Response, AppError> {
    let user: UserProfile = db.fetch_user(123).await?;

    // Example using the Maud templating engine (Pilcrow is template-agnostic)
    let markup = maud::html! {
        div.profile {
            h1 { (user.name) }
            p { (user.bio) }
        }
    }.into_string();

    pilcrow::respond!(req, {
        html => html(markup).with_toast("Loaded", "info"),
        json => raw user,
    })
}
```

Only the matching closure executes. The data fetch runs before `respond!` — it's shared across both arms.

### Multi-Target Update

When a single action needs to update multiple parts of the page:

```rust
pub async fn toggle_favorite(req: SilcrowRequest) -> Result<Response, AppError> {
    let item = db.toggle_favorite(item_id).await?;
    let count = db.favorites_count(user_id).await?;

    pilcrow::respond!(req, {
        html => html(render_item(&item))
            .patch_target("#fav-count", &serde_json::json!({"count": count}))
            .with_toast("Updated", "success"),
        json => json(item),
    })
}
```

### Real-Time Dashboard with SSE

```rust
pub const DASH_EVENTS: SseRoute = SseRoute::new("/events/dash");

pub async fn dashboard(req: SilcrowRequest) -> Result<Response, AppError> {
    let stats = db.get_stats().await?;
    let markup = render_dashboard(&stats);

    pilcrow::respond!(req, {
        html => html(markup).sse(DASH_EVENTS),
        json => json(stats),
    })
}

pub async fn dashboard_stream() -> impl IntoResponse {
    let stream = async_stream::stream! {
        loop {
            let stats = db.get_stats().await;
            yield Ok::<_, Infallible>(SilcrowEvent::patch(stats, "#dashboard").into());
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    };
    pilcrow::sse(stream)
}

// Router
Router::new()
    .route("/dashboard", get(dashboard))
    .route(DASH_EVENTS.path(), get(dashboard_stream))
```

### Real-Time Chat with WebSocket

```rust
pub const CHAT_WS: WsRoute = WsRoute::new("/ws/chat");

pub async fn chat(req: SilcrowRequest) -> Result<Response, AppError> {
    let history = db.recent_messages().await?;
    let markup = render_chat(&history);

    pilcrow::respond!(req, {
        html => html(markup).ws(CHAT_WS),
        json => json(history),
    })
}

pub async fn chat_handler(upgrade: WebSocketUpgrade) -> Response {
    pilcrow::ws::ws(upgrade, |mut stream| async move {
        while let Some(Ok(event)) = stream.recv().await {
            if let WsEvent::Custom { event: name, data } = event {
                let saved = db.save_message(&data).await;
                stream.send(WsEvent::html(
                    render_message(&saved), "#messages"
                )).await.ok();
            }
        }
    })
}

// Router
Router::new()
    .route("/chat", get(chat))
    .route(CHAT_WS.path(), get(chat_handler))
```

### Raw Shorthand

When you just need to return a struct without modifiers:

```rust
pub async fn get_user(req: SilcrowRequest) -> Result<Response, AppError> {
    let user = db.fetch_user(123).await?;
    let markup = render_user(&user);

    pilcrow::respond!(req, {
        html => html(markup),
        json => raw user,
    })
}
```

### Enums for Variant Responses

When an endpoint can return different shapes depending on logic:

```rust
#[derive(Serialize)]
#[serde(tag = "type")]
enum ApiResponse {
    Success { id: i64 },
    Error { reason: String },
}

pub async fn create_item(req: SilcrowRequest) -> Result<Response, AppError> {
    let result = db.create_item().await;

    let (markup, payload) = match result {
        Ok(id) => (
            format!("<p>Created item {id}</p>"),
            ApiResponse::Success { id },
        ),
        Err(e) => (
            format!("<p class='error'>{e}</p>"),
            ApiResponse::Error { reason: e.to_string() },
        ),
    };

    pilcrow::respond!(req, {
        html => html(markup),
        json => json(payload),
    })
}
```

## Public API

| Export | Description |
| --- | --- |
| `SilcrowRequest` | Axum extractor for content negotiation |
| `respond!` | Macro for declaring response arms |
| `html(data)` | HTML response constructor |
| `json(data)` | JSON response constructor |
| `navigate(path)` | Redirect response constructor (303) |
| `ResponseExt` | Modifier trait (`.with_toast()`, `.no_cache()`, `.sse()`, etc.) |
| `SseRoute` | Typed SSE route constant |
| `SilcrowEvent` | Structured SSE event builder (`.patch()`, `.html()`) |
| `sse(stream)` | Creates an SSE response from a stream with keep-alive |
| `WsRoute` | Typed WebSocket route constant |
| `WsEvent` | Bidirectional WebSocket message enum (`.patch()`, `.html()`, `.invalidate()`, `.navigate()`, `.custom()`) |
| `WsStream` | Typed WebSocket connection wrapper (`.send()`, `.recv()`, `.close()`) |
| `ws::ws(upgrade, handler)` | Upgrades HTTP to WebSocket with a typed handler |
| `Responses` | Builder for advanced use cases |

## License

MIT
