# Pilcrow

A response layer for [Axum](https://github.com/tokio-rs/axum) that turns handlers into multi-modal engines — one handler serves HTML to browsers and JSON to API clients via content negotiation, lazy evaluation, and server-side orchestration of the [Silcrow.js](public/SILCROW.md) frontend runtime.

## Quick Start

```rust
use pilcrow::*;

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

async fn home(req: SilcrowRequest) -> Result<Response, StatusCode> {
    req.select(Responses::new()
        .html(|| async { "<h1>Hello</h1>".to_string() })
        .json(|| async { serde_json::json!({"msg": "Hello"}) })
    ).await
}
```

## Why Pilcrow?

Raw Axum requires manual header parsing and format switching in every handler:

```rust
async fn handler(headers: HeaderMap) -> Response {
    let accept = headers.get("Accept").and_then(|v| v.to_str().ok()).unwrap_or("");
    if accept.contains("text/html") {
        Html("<h1>Hi</h1>").into_response()
    } else {
        Json(json!({"m": "Hi"})).into_response()
    }
}
```

Pilcrow replaces this with a declarative API where you register closures per format and the framework executes only the one the client needs:

```rust
async fn handler(req: SilcrowRequest) -> Result<Response, StatusCode> {
    req.select(Responses::new()
        .html(|| async { "<h1>Hi</h1>".to_string() })
        .json(|| async { serde_json::json!({"m": "Hi"}) })
    ).await
}
```

Content negotiation, lazy evaluation, and response packaging are handled for you.

## Core Concepts

### 1. The Extractor: `SilcrowRequest`

An Axum extractor that reads the `Accept` and `silcrow-target` headers to determine what the client wants. Use it as a handler argument — Axum injects it automatically.

```rust
pub async fn handler(req: SilcrowRequest) -> Result<Response, AppError> {
    // req.preferred_mode() returns RequestMode::Html or RequestMode::Json
    // req.select(...) dispatches to the right closure
}
```

The negotiation logic: Silcrow.js requests respect `Accept` strictly. Standard browser requests default to HTML. Everything else falls back to JSON.

### 2. The Selector: `req.select(Responses)`

`Responses` is a builder where you register async closures for each format. `select()` evaluates the client's preferred mode and runs **only** the matching closure — the other is never executed.

```rust
req.select(Responses::new()
    .html(|| async { /* only runs for HTML clients */ })
    .json(|| async { /* only runs for JSON clients */ })
).await
```

If the client requests a format you didn't register, Pilcrow returns `406 Not Acceptable` automatically.

### 3. Three Levels of Response

Closures support three return styles, from zero-boilerplate to full control:

**Level 1 — Pure data.** Return a `String` for HTML or a `serde_json::Value` for JSON. Pilcrow wraps it.

```rust
.html(|| async { "<h1>Dashboard</h1>".to_string() })
.json(|| async { json!({"status": "online"}) })
```

**Level 2 — Fallible data.** Return a `Result`. Errors propagate via `?`.

```rust
.html(|| async {
    let user = db.get_user(id).await?;
    Ok(format!("<h1>{}</h1>", user.name))
})
```

**Level 3 — Full package.** Use the `html()` / `json()` constructors to access modifiers.

```rust
.html(|| async {
    Ok(html(markup)
        .with_toast("Updated", "success")
        .no_cache())
})
```

### 4. Modifiers via `ResponseExt`

All response types (`HtmlResponse`, `JsonResponse`, `NavigateResponse`) implement `ResponseExt`, giving you a unified modifier chain:

| Method | Effect |
| --- | --- |
| `.with_toast(msg, level)` | Cookie-based for HTML/Navigate, payload-injected for JSON |
| `.with_header(key, value)` | Arbitrary response header |
| `.no_cache()` | Sets `silcrow-cache: no-cache` to prevent client caching |
| `.retarget(selector)` | Silcrow.js swaps HTML into a different DOM element |
| `.trigger_event(name)` | Fires a `CustomEvent` in the browser via Silcrow.js |
| `.push_history(url)` | Updates the browser URL bar without a page load |

Toast transport is automatic — HTML responses use a short-lived cookie (`Max-Age=5`, `SameSite=Lax`), JSON responses inject a `_toasts` array into the payload. If the JSON root isn't an object (e.g. you returned a `Vec`), Pilcrow wraps it as `{"data": [...], "_toasts": [...]}`.

### 5. Navigation (Redirects)

Redirects are imperative — they're not negotiated. Use `navigate()` for early returns like auth guards:

```rust
pub async fn admin(req: SilcrowRequest) -> Result<Response, AppError> {
    if !user.is_admin() {
        return Ok(navigate("/login")
            .with_toast("Unauthorized", "error")
            .into_response());
    }

    req.select(Responses::new()
        .html(|| async { admin_markup.to_string() })
    ).await
}
```

`navigate()` returns a `303 See Other` with the `Location` header. Toasts persist across the redirect via cookie.

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

## Full Example: Dual-Mode Handler with DB

```rust
pub async fn get_profile(
    req: SilcrowRequest,
    State(db): State<DbPool>,
) -> Result<Response, AppError> {
    let user = db.fetch_user(123).await?;

    req.select(Responses::new()
        .html(|| async {
            Ok(html(maud::html! {
                div.profile {
                    h1 { (user.name) }
                    p { (user.bio) }
                }
            }.into_string())
            .with_toast("Loaded", "info"))
        })
        .json(|| async {
            Ok(json(serde_json::json!({
                "id": user.id,
                "name": user.name
            })))
        })
    ).await
}
```

Both closures are async and only the one matching the client's `Accept` header executes. The data fetch (`db.fetch_user`) is shared — it runs before `select()`. If you need format-specific queries, put them inside the closures.

## Dependencies

```toml
[dependencies]
pilcrow = "0.1"
```

Pilcrow depends on `axum 0.7`, `serde`, `serde_json`, `cookie`, `urlencoding`, and `tracing`. No runtime overhead beyond what Axum already requires.

## License

MIT
