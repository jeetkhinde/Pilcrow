# Pilcrow

A response layer for [Axum](https://github.com/tokio-rs/axum) that turns handlers into multi-modal engines — one handler serves HTML to browsers and JSON to API clients via content negotiation, lazy evaluation, and server-side orchestration of the [Silcrow.js](public/SILCROW.md) frontend runtime.

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

Content negotiation, lazy evaluation, and response packaging are handled for you.

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

The macro expands to `req.select(Responses::new()...).await` — the builder pattern runs underneath, but you don't write the boilerplate.

**JSON shorthand with `raw`:** For serializable values where you don't need modifiers, skip the `json()` constructor:

```rust
pilcrow::respond!(req, {
    html => html(markup),
    json => raw user,        // auto-wrapped in json()
})
```

`raw` is JSON-only. HTML always uses the explicit `html()` constructor.

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

### 3. The Builder API (Advanced)

The `respond!` macro covers 95% of cases. For the remaining 5% — conditional logic inside response arms, complex async workflows — use the builder directly:

```rust
req.select(Responses::new()
    .html(move || async move {
        if user.is_premium {
            html(premium_markup).with_header("X-Tier", "premium")
        } else {
            html(basic_markup)
        }
    })
    .json(move || async move { json(user) })
).await
```

The builder supports three return styles per closure:

**Level 1 — Pure data.** Return a `String` for HTML or a serializable value for JSON. Pilcrow wraps it.

```rust
.html(|| async { "<h1>Dashboard</h1>".to_string() })
.json(|| async { user })
```

**Level 2 — Fallible data.** Return a `Result`. Errors propagate via `?`.

```rust
.html(|| async {
    let user = db.get_user(id).await?;
    Ok(format!("<h1>{}</h1>", user.name))
})
```

**Level 3 — Full package.** Use constructors with modifiers.

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

    pilcrow::respond!(req, {
        html => html(admin_markup),
    })
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

    let markup = maud::html! {
        div.profile {
            h1 { (user.name) }
            p { (user.bio) }
        }
    }.into_string();

    pilcrow::respond!(req, {
        html => html(markup).with_toast("Loaded", "info"),
        json => json(user),
    })
}
```

Only the matching closure executes. The data fetch (`db.fetch_user`) runs before `respond!` — it's shared. If you need format-specific queries, drop to the builder API and put them inside the closures.

## Patterns for Beginners

### One-Off Responses

For simple endpoints that don't map to a database model, define a small struct:

```rust
#[derive(Serialize)]
struct StatusResponse {
    status: String,
}

pub async fn health(req: SilcrowRequest) -> Result<Response, StatusCode> {
    pilcrow::respond!(req, {
        html => html("<p>OK</p>"),
        json => json(StatusResponse { status: "ok".into() }),
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

### Raw Shorthand

When you just need to return a struct without modifiers:

```rust
pub async fn get_user(req: SilcrowRequest) -> Result<Response, AppError> {
    let user = db.fetch_user(123).await?;
    let markup = render_user(&user);

    pilcrow::respond!(req, {
        html => html(markup),
        json => raw user,    // equivalent to json(user), no chaining needed
    })
}
```

## Dependencies

```toml
[dependencies]
pilcrow = "0.1"
```

Pilcrow depends on `axum 0.7`, `serde`, `serde_json`, `cookie`, `urlencoding`, and `tracing`. No runtime overhead beyond what Axum already requires.

## License

MIT
