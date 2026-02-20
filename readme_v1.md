# 📖 How to Use Pilcrow: The Premium Axum Dashboard

**Pilcrow** is an ergonomic response layer for [Axum](https://github.com/tokio-rs/axum). It’s designed to bridge the gap between your Rust backend and the **Silcrow.js** frontend, turning messy HTTP negotiation into a clean, value-first API.

---

## 1. The Comparison: Axum vs. Pilcrow

### The "Traditional" Axum Way

To support both a web page (HTML) and an API (JSON) in one handler, you usually have to parse headers manually. It’s imperative, repetitive, and error-prone.

```rust
async fn handler(headers: HeaderMap) -> impl IntoResponse {
    let accept = headers.get("Accept").and_then(|v| v.to_str().ok()).unwrap_or("");
    
    if accept.contains("text/html") {
        Html("<h1>Hello</h1>").into_response()
    } else {
        Json(json!({"msg": "Hello"})).into_response()
    }
}

```

### The Pilcrow Way

Pilcrow uses **Content Negotiation**. You declare your intentions, and Pilcrow executes the right one lazily.

```rust
async fn handler(req: SilcrowRequest) -> Result<Response, AppError> {
    req.select(Responses::new()
        .html(|| Ok(html("<h1>Hello</h1>")))
        .json(|| Ok(json(json!({"msg": "Hello"}))))
    )
}

```

---

## 2. Returning Dual Responses (HTML + JSON)

In a modern hypermedia app, your frontend often asks for **HTML** to swap into the DOM, but your mobile app or API clients ask for **JSON**. Pilcrow handles this in one block.

**Scenario:** Fetching a user from a database and returning either a partial profile or a data object.

```rust
pub async fn get_user_profile(
    req: SilcrowRequest,
    State(db): State<DbPool>,
) -> Result<Response, AppError> {
    // 1. Fetch data (Shared across both modes)
    let user = db.fetch_user(123).await?; 

    // 2. Negotiate Response
    req.select(Responses::new()
        .html(|| {
            // This closure ONLY runs if the client asks for HTML
            Ok(html(maud::html! {
                div.profile {
                    h1 { (user.name) }
                    p { (user.bio) }
                }
            }))
        })
        .json(|| {
            // This closure ONLY runs if the client asks for JSON
            Ok(json(serde_json::json!({
                "id": user.id,
                "name": user.name,
                "role": "admin"
            })))
        })
    )
}

```

---

## 3. Targeted Responses (HTML or JSON only)

Sometimes an endpoint is *only* meant for Silcrow's DOM patching or *only* meant for data. If the client asks for the wrong format, Pilcrow automatically returns a **406 Not Acceptable**.

```rust
// This handler ONLY responds to JSON requests
pub async fn update_settings(req: SilcrowRequest) -> Result<Response, AppError> {
    req.select(Responses::new()
        .json(|| Ok(json(json!({"status": "updated"}))))
    )
}

```

---

## 4. Toasts & Modifiers (The Premium DX)

Pilcrow unifies modifiers through the `ResponseExt` trait. You use the same methods regardless of the output format.

* **HTML/Navigate:** Toasts are sent via secure **Cookies** (survives redirects).
* **JSON:** Toasts are injected into the **JSON payload** (`_toasts: [...]`).

```rust
req.select(Responses::new()
    .html(|| {
        Ok(html(markup)
            .with_toast("Success!", "success") // Automatic Cookie
            .with_header("X-Custom", "Value")
            .no_cache())
    })
    .json(|| {
        Ok(json(data)
            .with_toast("Success!", "success")) // Automatic JSON injection
    })
)

```

---

## 5. Navigation & Early Bailouts

Redirects are **imperative**. They are not negotiated; the server just decides to move the user. In Pilcrow, we use `Maps(path)` for this.

```rust
pub async fn protected_route(req: SilcrowRequest) -> Result<Response, AppError> {
    if !user.is_admin() {
        // Use .into_response() to cast the builder to a standard Axum response
        return Ok(navigate("/login")
            .with_toast("Unauthorized Access", "error")
            .into_response());
    }

    req.select(Responses::new().html(|| Ok(html(admin_panel))))
}

```

---

## 6. Advanced Client Control (Retargeting & Events)

You can control the **Silcrow.js** frontend directly from Rust using custom headers.

```rust
Ok(html(markup)
    // Tell Silcrow to swap this HTML into #modal instead of the default target
    .retarget("#modal") 
    // Tell Silcrow to fire a custom DOM event 'user-updated'
    .trigger_event("user-updated")
    // Force the browser URL to change
    .push_history("/profile/settings"))

```

---

## 7. Layouts & Routing in Axum

### App Structure

Pilcrow works best when you serve the Silcrow JS bundle via the built-in asset handler.

```rust
use pilcrow::assets::{serve_silcrow_js, SILCROW_JS_PATH, script_tag};

#[tokio::main]
async fn main() {
    let app = Router::new()
        // 1. Serve the Silcrow runtime
        .route(SILCROW_JS_PATH, get(serve_silcrow_js))
        // 2. Grouped Routes
        .nest("/api", api_routes())
        .route("/", get(home_handler));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

```

### Composing Layouts

Since Pilcrow is template-agnostic, you can use a simple Rust function to wrap your "partials."

```rust
fn base_layout(title: &str, content: maud::Markup) -> maud::Markup {
    maud::html! {
        (maud::DOCTYPE)
        html {
            head {
                title { (title) }
                // Use Pilcrow's script_tag() to include the JS automatically
                (maud::PreEscaped(script_tag()))
            }
            body { (content) }
        }
    }
}

```

---

## 💡 Summary Checklist for Developers

1. **Extract:** Use `SilcrowRequest` in your handler arguments.
2. **Bail Early:** Use `Maps(path).into_response()` for redirects/auth.
3. **Negotiate:** Use `req.select(Responses::new()...)` for the main response.
4. **Modify:** Chain `.with_toast()` or `.no_cache()` at the end of your `html()` or `json()` calls.
5. **Return:** Always wrap your result in `Ok(...)`.

**Pilcrow** ensures that whether you are returning a small HTML partial for a button click or a full JSON object for a mobile app, your code remains clean, dry, and professional.