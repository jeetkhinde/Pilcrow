# 🏆 The Definitive Guide to Pilcrow

**Pilcrow** is a premium response layer for Axum. It turns standard HTTP handlers into powerful, multi-modal engines that orchestrate the **Silcrow.js** frontend via content negotiation and server-side instructions.

---

## 1. The Core Philosophy: "Intelligent Packaging"

In a Pilcrow app, the backend doesn't just send "data"; it sends a **Managed Response**.

* **Content Negotiation:** One handler serves HTML to your browser and JSON to your mobile app automatically.
* **Lazy Evaluation:** Only the code required for the specific client format is executed.
* **Zero-Friction API:** Return raw data or full response packages—Pilcrow's trait system handles the wrapping.

---

## 2. From Axum to Pilcrow: The Evolution

### The Boilerplate Way (Raw Axum)

You have to manually check headers and handle different return types.

```rust
async fn handler(headers: HeaderMap) -> Response {
    if is_html(&headers) {
        Html("<h1>Hi</h1>").into_response()
    } else {
        Json(json!({"m": "Hi"})).into_response()
    }
}

```

### The Fluent Way (Pilcrow)

The `SilcrowRequest` extractor handles the "Check," and `req.select` handles the "Response."

```rust
async fn handler(req: SilcrowRequest) -> Result<Response, AppError> {
    req.select(Responses::new()
        .html(|| "<h1>Hi</h1>")
        .json(|| json!({"m": "Hi"}))
    )
}

```

---

## 3. The Fluent API: 3 Levels of Data Returning

Pilcrow’s V3 API uses **Polymorphic Inference**. You don't have to wrap everything in `Ok(html(...))` anymore.

### Level 1: Pure Data (Zero Boilerplate)

If the data is ready, just return it. Pilcrow assumes success.

```rust
.html(|| maud::html! { h1 { "Dashboard" } }.into_string())
.json(|| json!({ "status": "online" }))

```

### Level 2: Fallible Data (The `?` Operator)

If you are fetching from a DB, return a `Result`. Pilcrow handles the mapping.

```rust
.html(|| {
    let user = db.get_user(id)?; // Returns Result<User, E>
    Ok(format!("<h1>Welcome, {}</h1>", user.name))
})

```

### Level 3: Full Package (Modifiers)

When you need to send Toasts or use advanced orchestration, use the `html()` or `json()` constructors.

```rust
.html(|| {
    Ok(html(markup)
        .with_toast("Profile Updated", "success")
        .no_cache())
})

```

---

## 4. Server-Side Orchestration (HTMX Power)

Pilcrow allows the Rust backend to act as a "Puppet Master" for the Silcrow.js frontend using invisible HTTP headers.

```rust
pub async fn save_handler(req: SilcrowRequest) -> Result<Response, AppError> {
    req.select(Responses::new()
        .html(|| {
            Ok(html(success_partial)
                .with_toast("Saved!", "success")
                // Hijack the swap: put this HTML in #sidebar instead of the original target
                .retarget("#sidebar") 
                // Tell JS to fire a 'refresh-data' event
                .trigger_event("refresh-data")
                // Update the browser's URL bar without a full page load
                .push_history("/dashboard/success"))
        })
    )
}

```

---

## 5. Navigation: The Imperative Bailout

Redirects are not "negotiated"—they are server decisions. Use `Maps(path)` for early returns in your logic (like Auth or Errors).

```rust
pub async fn admin_panel(req: SilcrowRequest) -> Result<Response, AppError> {
    // 1. Early Return (The Bailout)
    if !user.is_admin() {
        return Ok(navigate("/login")
            .with_toast("Admins Only!", "error")
            .into_response());
    }

    // 2. Negotiated Response
    req.select(Responses::new().html(|| admin_markup))
}

```

---

## 6. Layout Composition & Setup

### Serve the Asset

Pilcrow provides an embedded handler for the JS runtime so you don't have to manage files.

```rust
use pilcrow::assets::{serve_silcrow_js, SILCROW_JS_PATH};

let app = Router::new()
    .route(SILCROW_JS_PATH, get(serve_silcrow_js))
    .route("/", get(home));

```

### Build the Shell

Use the `script_tag()` helper to inject the runtime into your base layout. It returns a `&'static str`, making it compatible with any template engine.

```rust
fn site_shell(content: String) -> String {
    format!(r#"
        <html>
            <head>{}</head>
            <body>{}</body>
        </html>
    "#, pilcrow::assets::script_tag(), content)
}

```

---

## 💡 Developer Summary

| Feature | Method | Result |
| --- | --- | --- |
| **Toast** | `.with_toast(msg, lvl)` | Cookie (HTML) or Payload (JSON) |
| **Redirect** | `Maps(url)` | 303 Redirect with Toast persistence |
| **Swap Target** | `.retarget(selector)` | Silcrow.js swaps into specific element |
| **DOM Events** | `.trigger_event(name)` | Fires `CustomEvent` in browser |
| **URL Change** | `.push_history(url)` | Updates browser history bar |
| **Cache Control** | `.no_cache()` | Prevents Silcrow.js from caching the view |

**Pilcrow** takes the complexity of modern web state management and hides it behind a clean, type-safe Rust API. One handler, any client, total control.