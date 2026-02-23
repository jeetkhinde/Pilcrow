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

The comparison between a raw **Axum + Maud** setup and the **Pilcrow** experience is the difference between building a car from parts versus driving a luxury vehicle with an intelligent dashboard.

While Axum provides the engine and Maud provides the cargo, Pilcrow acts as the orchestration layer that makes them talk to each other and to the frontend.

---

### 1. The Developer Workflow: A Side-by-Side

**The Task:** Create an endpoint that saves a user profile. It must return a partial HTML div for the web frontend, JSON for the mobile app, and a "Success" toast message that survives a redirect if needed.

#### The Axum + Maud Way (Manual Labor)

You are responsible for every HTTP detail. You have to check headers manually and handle "state" like toasts via string manipulation.

```rust
async fn save_user(headers: HeaderMap, State(db): State<DbPool>) -> Response {
    let user = db.save().await.unwrap();
    let accept = headers.get("Accept").and_then(|v| v.to_str().ok()).unwrap_or("");

    if accept.contains("text/html") {
        let cookie = "silcrow_toast=Saved:success; Path=/; SameSite=Lax";
        (
            StatusCode::OK,
            [(header::SET_COOKIE, cookie)],
            Html(maud::html! { div { (user.name) " saved!" } }.into_string())
        ).into_response()
    } else {
        Json(json!({ "status": "success", "user": user, "_toast": "Saved" })).into_response()
    }
}

```

* **The Pain:** High boilerplate. You have to remember to inject the toast differently for JSON vs HTML. Negotiation logic is repeated in every handler.

#### The Pilcrow Way (Fluent Orchestration)

You declare your intent. Pilcrow handles the "How" of the HTTP transport.

```rust
async fn save_user(req: SilcrowRequest, State(db): State<DbPool>) -> Result<Response, E> {
    let user = db.save().await?;

    req.select(Responses::new()
        .html(|| html(maud::html! { div { (user.name) " saved!" } }))
        .json(|| json!(user))
    ).map(|res| res.with_toast("Saved!", "success")) 
}

```

* **The Gain:** The `with_toast` modifier works globally. The negotiation is handled by the framework. The code focuses entirely on data and UI.

---

### 2. Feature Comparison Table

| Feature | Axum + Maud | Pilcrow Experience |
| --- | --- | --- |
| **Content Negotiation** | Manual `Accept` header parsing in every function. | Automated via `req.select()`. |
| **Execution** | Eager (you fetch data before knowing if the client wants it). | Lazy (only the required closure runs). |
| **Toasts/Alerts** | Manual cookie formatting or JSON injection. | Unified `.with_toast()` modifier. |
| **Frontend Sync** | None. You manually write JS to handle updates. | Deep sync with `silcrow.js` via `.retarget()` and `.trigger_event()`. |
| **Error Handling** | Manual mapping to `StatusCode`. | Unified `AppError` and `?` support inside closures. |
| **Redirects** | `Redirect::to(...)` (Toasts often lost). | `Maps(...)` (Toasts persisted via safe cookies). |

---

### 3. The "Vibe" Shift: From Components to Orchestration

#### Axum + Maud is "Component-First"

You think in terms of **Response Types**. Every time you write a handler, you are asking: *"What specific HTTP object do I need to construct right now?"* This leads to "Fragmented Logic," where your JSON API and your HTML views live in different worlds, even if they do the same thing.

#### Pilcrow is "UI-First"

You think in terms of **Interactions**. You are asking: *"What is the result of this action, and how should the UI (in any format) reflect it?"* Because Pilcrow handles the "packaging," you can spend your time building complex UI patterns:

* **"Save this form, but update the sidebar too."** (`.retarget("#sidebar")`)
* **"Delete this item, and tell the header to refresh the count."** (`.trigger_event("update-cart")`)
* **"Update the profile, and make sure the URL bar matches."** (`.push_history("/profile")`)

### 🏁 Final Verdict

* **Axum + Maud** is for when you want a standard, "by-the-book" REST API and simple server-rendered pages. It is robust but requires you to do the heavy lifting for modern UX.
* **Pilcrow** is for developers building **Hypermedia Applications**. It gives you the feel of a single-page app (SPA) with the simplicity of a multi-page app (MPA). It’s for when you want "framework-grade" features like global toasts and DOM retargeting without the weight of a heavy frontend framework.
