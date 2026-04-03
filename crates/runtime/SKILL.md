---
name: pilcrow-silcrow
description: >
  Guide for developing Pilcrow (Rust/Axum response layer) and Silcrow.js (client-side runtime). 
  Use this skill for any work on content negotiation, response building, SSE/WebSocket, DOM patching, 
  client-side navigation, toast systems, or the build pipeline. Trigger whenever the user mentions 
  Pilcrow, Silcrow, respond! macro, ResponseExt, SseRoute, WsRoute, SilcrowEvent, WsEvent, or any 
  of the silcrow/*.js modules.
---

# Pilcrow + Silcrow.js

## Architecture

**Pilcrow** (Rust/Axum) — `src/`
**Silcrow.js** (client IIFE) — `silcrow/*.js` → bundled by `build.rs` → `public/silcrow.js`

### File Map

```text
src/lib.rs        — re-exports only
src/extract.rs    — SilcrowRequest extractor + RequestMode enum
src/response.rs   — html(), json(), navigate(), ResponseExt trait, BaseResponse
src/select.rs     — Responses builder + IntoPilcrowHtml/Json traits
src/macros.rs     — respond! macro (12 arms)
src/sse.rs        — SseRoute, SilcrowEvent, sse() helper
src/ws.rs         — WsRoute, WsEvent, WsStream, WsRecvError, ws() upgrade helper
src/assets.rs     — serve_silcrow_js(), silcrow_js_path(), script_tag()
silcrow/debug.js
silcrow/patcher.js
silcrow/safety.js
silcrow/toasts.js
silcrow/navigator.js
silcrow/live.js
silcrow/ws.js
silcrow/optimistic.js
silcrow/index.js
build.rs          — JS concat + hash + release minification
tests/macro_usage.rs
```

JS module order in `build.rs` is load-order — dependencies before dependents.

## Design Philosophy

**Compute pure, apply once.**

Pilcrow is a thin response layer, not a framework. Axum owns routing, extractors, middleware, and state. Pilcrow adds content negotiation and server-driven UI orchestration through pure data transformations that resolve into Axum responses at the boundary.

Every function falls into one of two categories:

- **Pure transforms** — data in, data out. No mutation, no side effects. These are the majority: toast building, JSON merging, header value construction, content negotiation, event serialization.
- **Axum boundary** — the thin imperative seam where pure results become `Response`. Limited to: `IntoResponse` impls, `apply_to_response(&mut Response)`, extractor `from_request_parts`.

This split means bugs live in testable pure functions, and the Axum seam is small enough to audit by inspection.

## Functional Programming Rules

**Prefer `Option`/`Result` chains over `if/else` trees.**

```rust
// Yes
serde_json::to_string(&self.toasts)
    .ok()
    .map(|json| urlencoding::encode(&json).into_owned())
    .map(|encoded| Cookie::build(("silcrow_toasts", encoded)).path("/").build())
    .and_then(|cookie| HeaderValue::from_str(&cookie.to_string()).ok())

// No
if !self.toasts.is_empty() {
    let json = serde_json::to_string(&self.toasts);
    if let Ok(json) = json { ... }
}
```

**Extract repeated logic into small composable functions.**
A pattern used 3+ times becomes a named function. Inline closures are fine for one-off transforms in chains.

**Data in, data out — no `&mut self` in transform functions.**
Transform functions take owned or borrowed data and return new values. The only methods that take `&mut` are the Axum boundary methods that apply computed results to a `Response`.

**Side effects isolated to named apply functions.**
All mutation of `Response` happens in clearly named methods: `apply_to_response`, `apply_toast_cookies`. Never scatter `headers_mut().insert(...)` across transform logic.

**Serialization separate from application.**
Build the value first (pure), then set the header (boundary). Never serialize inside a modifier chain.

**Anti-patterns to avoid:**

- Serialization inside a modifier chain (separate compute from apply)
- Duplicated `if let Ok(val) = HeaderValue::from_str(...)` — use a shared helper
- Mixed pure logic and mutation in the same function body
- `unwrap()` or `expect()` anywhere in library code

## Public API Surface

```rust
use pilcrow::*;
// Exposes: SilcrowRequest, respond!, html(), json(), navigate(), ResponseExt,
//          Responses, SseRoute, SilcrowEvent, sse(),
//          WsRoute, WsEvent, WsStream,
//          StatusCode, Response, axum
```

## respond! Macro — All Arms

```rust
// Both arms
respond!(req, { html => expr, json => expr })
respond!(req, { html => expr, json => raw expr })          // raw: auto-wraps in json()

// With shared toast (applied to whichever branch runs)
respond!(req, { html => expr, json => expr, toast => (msg, level) })
respond!(req, { html => expr, json => raw expr, toast => (msg, level) })

// Single arm (other returns 406)
respond!(req, { html => expr })
respond!(req, { json => expr })
respond!(req, { json => raw expr })

// Single arm + toast
respond!(req, { html => expr, toast => (msg, level) })
respond!(req, { json => expr, toast => (msg, level) })
respond!(req, { json => raw expr, toast => (msg, level) })
```

`raw` is JSON-only. HTML always uses explicit `html()`. Builder is the escape hatch when macro isn't enough.

## ResponseExt Modifiers (chainable, infallible)

| Method | Header set |
| --- | --- |
| `.with_toast(msg, level)` | Cookie (HTML/Navigate), `_toasts` injection (JSON) |
| `.no_cache()` | `silcrow-cache: no-cache` |
| `.retarget(selector)` | `silcrow-retarget` |
| `.trigger_event(name)` | `silcrow-trigger` |
| `.push_history(url)` | `silcrow-push` |
| `.patch_target(selector, data)` | `silcrow-patch` (JSON payload) |
| `.invalidate_target(selector)` | `silcrow-invalidate` |
| `.client_navigate(path)` | `silcrow-navigate` |
| `.sse(path)` | `silcrow-sse` |
| `.ws(path)` | `silcrow-ws` |

All modifiers silently no-op on invalid input with `tracing::warn` in debug. Side-effect headers execute client-side in order: patch → invalidate → navigate → sse/ws.

## SSE Pattern

```rust
pub const FEED: SseRoute = SseRoute::new("/events/feed");

// In handler: .sse(FEED) or .sse(FEED.path())
// SilcrowEvent::patch(data, "#target") or SilcrowEvent::html(markup, "#target")
// pilcrow::sse(stream) wraps Axum Sse with keep-alive
```

## WebSocket Pattern

```rust
pub const CHAT: WsRoute = WsRoute::new("/ws/chat");

// In handler: .ws(CHAT) or .ws(CHAT.path())
// WsEvent variants: patch, html, invalidate, navigate, custom
// pilcrow::ws::ws(upgrade, |mut stream| async move { ... })
```

## Content Negotiation Logic

`SilcrowRequest::preferred_mode()`:

- Silcrow AJAX (`silcrow-target` header present) → respect `Accept` header
- Standard browser (no `silcrow-target`) → default HTML
- No match → JSON fallback

## Safety Rules (non-negotiable)

**Rust:**

- No `unwrap()` in library code. Use `?` or explicit match.
- All user strings validated before `HeaderValue::from_str()`.
- Cookie values URL-encoded via `urlencoding::encode`.
- JSON serialization via `serde_json::to_value` with 500 fallback — never panic.
- Async closures in `Responses` builder must be `Send + 'static`.
- All public types implement `Debug` (manual impl where derive isn't possible).

**JS:**

- All DOM mutation via `safeSetHTML()` or `textContent`. Never raw `innerHTML`.
- `on*` attributes stripped in all template/HTML paths.
- Prototype pollution paths (`__proto__`, `constructor`, `prototype`) blocked in `resolvePath()`.
- Public API on `window.Silcrow` only. Internal functions are IIFE-scoped.
- `warn()` in production, `throwErr()` only in `s-debug` mode.

## Code Style

- Rust: Prefer pure functions returning values; isolate mutation to Axum boundary methods.
- Rust: `impl Into<String>` for constructors, `&str` for modifier params, `impl AsRef<str>` for both.
- Rust: Tests in `#[cfg(test)] mod tests` per file. Integration tests in `tests/`.
- Rust: No `unwrap()` or `expect()` in library code (only in tests and `build.rs`).
- JS: `const` over `let`. No classes. No `this` outside `window.Silcrow` literal.
- JS: Named functions (not arrows) in event listeners for debugging clarity.
- JS: Modules are separate files, concatenated by `build.rs`. Module order matters.

## Common Patterns

### Dual-mode handler

```rust
async fn handler(req: SilcrowRequest, State(db): State<Db>) -> Result<Response, AppError> {
    let data = db.fetch().await?;
    respond!(req, {
        html => html(render(&data)).with_toast("Loaded", "info"),
        json => raw data,
    })
}
```

### Multi-target update

```rust
html(render_item(&item))
    .patch_target("#count", &json!({"count": count}))
    .invalidate_target("#sidebar")
    .with_toast("Saved", "success")
```

### Auth guard

```rust
if !user.is_admin() {
    return Ok(navigate("/login").with_toast("Unauthorized", "error").into_response());
}
```

### SSE stream

```rust
async fn stream_handler() -> impl IntoResponse {
    let stream = async_stream::stream! {
        loop {
            yield Ok::<_, Infallible>(SilcrowEvent::patch(data, "#feed").into());
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    };
    pilcrow::sse(stream)
}
```

### WebSocket handler

```rust
async fn ws_handler(upgrade: WebSocketUpgrade) -> Response {
    pilcrow::ws::ws(upgrade, |mut stream| async move {
        stream.send(WsEvent::patch(json!({"ready": true}), "#app")).await.ok();
        while let Some(Ok(event)) = stream.recv().await {
            // handle events
        }
    })
}
```

## Git / Commit Conventions

Branch: `{type}/{short-description}` (feat, fix, refactor, docs, test, chore)
Commits: `type(scope): description` — scope matches file map key (extract, response, select, macros, sse, ws, assets, silcrow, build, docs)
One logical change per commit. Every commit compiles and passes tests.

## Response Format Rules

- Questions → answer only, no code unless asked.
- Design decisions → options + recommendation + tradeoff in ≤10 lines.
- Code changes → diff only (file + function + change), not full file.
- New features → public API surface first, wait for approval, then implement.
- Code reviews → 🔴 bug / 🟡 safety / 🔵 DX / ⚪ nitpick, ordered by severity.
- Never generate code for TODO items unless explicitly asked.
- End code changes with branch + commit suggestion block.
