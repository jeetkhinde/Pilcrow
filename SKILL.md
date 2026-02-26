---
name: skill-creator
description: Guide for creating and updating Codex skills.
---

# Pilcrow + Silcrow.js

## Architecture

**Pilcrow** (Rust/Axum) — `crates/pilcrow/src/`
**Silcrow.js** (client IIFE) — `silcrow/*.js` → bundled by `build.rs` → `public/silcrow.js`

### File Map

```
src/lib.rs        — re-exports only
src/extract.rs    — SilcrowRequest extractor + RequestMode enum
src/response.rs   — html(), json(), navigate(), ResponseExt trait, BaseResponse
src/select.rs     — Responses builder + IntoPilcrowHtml/Json traits
src/macros.rs     — respond! macro (12 arms)
src/sse.rs        — SseRoute, SilcrowEvent, sse() helper
src/assets.rs     — serve_silcrow_js(), silcrow_js_path(), script_tag()
silcrow/debug.js
silcrow/patcher.js
silcrow/safety.js
silcrow/toasts.js
silcrow/navigator.js
silcrow/live.js
silcrow/optimistic.js
silcrow/index.js
build.rs          — JS concat + hash + release minification
tests/macro_usage.rs
```

JS module order in `build.rs` is load-order — dependencies before dependents.

## Public API Surface

```rust
use pilcrow::*;
// Exposes: SilcrowRequest, respond!, html(), json(), navigate(), ResponseExt,
//          Responses, SseRoute, SilcrowEvent, sse(), StatusCode, Response, axum
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
|---|---|
| `.with_toast(msg, level)` | Cookie (HTML/Navigate), `_toasts` injection (JSON) |
| `.no_cache()` | `silcrow-cache: no-cache` |
| `.retarget(selector)` | `silcrow-retarget` |
| `.trigger_event(name)` | `silcrow-trigger` |
| `.push_history(url)` | `silcrow-push` |
| `.patch_target(selector, data)` | `silcrow-patch` (JSON payload) |
| `.invalidate_target(selector)` | `silcrow-invalidate` |
| `.client_navigate(path)` | `silcrow-navigate` |
| `.sse(path)` | `silcrow-sse` |

Side-effect headers execute client-side in order: patch → invalidate → navigate → sse.

## SSE Pattern

```rust
pub const FEED: SseRoute = SseRoute::new("/events/feed");

// In handler: .sse(FEED) or .sse(FEED.path())
// SilcrowEvent::patch(data, "#target") or SilcrowEvent::html(markup, "#target")
// pilcrow::sse(stream) wraps Axum Sse with keep-alive
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

**JS:**
- All DOM mutation via `safeSetHTML()` or `textContent`. Never raw `innerHTML`.
- `on*` attributes stripped in all template/HTML paths.
- Prototype pollution paths (`__proto__`, `constructor`, `prototype`) blocked in `resolvePath()`.
- Public API on `window.Silcrow` only. Internal functions are IIFE-scoped.
- `warn()` in production, `throwErr()` only in `s-debug` mode.

## Code Style

- Rust: `impl Into<String>` for constructors, `&str` for modifier params, `impl AsRef<str>` for both.
- Rust: Tests in `#[cfg(test)] mod tests` per file. Integration tests in `tests/`.
- JS: `const` over `let`. No classes. No `this` outside `window.Silcrow` literal. Named functions (not arrows) in event listeners.

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

## Git / Commit Conventions

Branch: `{type}/{short-description}` (feat, fix, refactor, docs, test, chore)  
Commits: `type(scope): description` — scope matches file map key (extract, response, select, macros, sse, assets, silcrow, build, docs)  
One logical change per commit. Every commit compiles and passes tests.

## Response Format Rules

- Questions → answer only, no code unless asked.
- Design decisions → options + recommendation + tradeoff in ≤10 lines.
- Code changes → diff only (file + function + change), not full file.
- New features → public API surface first, wait for approval, then implement.
- Code reviews → 🔴 bug / 🟡 safety / 🔵 DX / ⚪ nitpick, ordered by severity.
- Never generate code for TODO items unless explicitly asked.
- End code changes with branch + commit suggestion block.
