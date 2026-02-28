# Response Modifiers

Every response type (`HtmlResponse`, `JsonResponse`, `NavigateResponse`) implements the `ResponseExt` trait, giving you a unified modifier chain. This guide covers every method with examples.

## Quick Reference

| Method | Header Set | Effect |
| --- | --- | --- |
| `.with_toast(msg, level)` | Cookie / `_toasts` | Show notification |
| `.with_header(key, value)` | Custom | Arbitrary response header |
| `.no_cache()` | `silcrow-cache` | Prevent client caching |
| `.retarget(selector)` | `silcrow-retarget` | Swap into different element |
| `.push_history(url)` | `silcrow-push` | Override browser URL |
| `.trigger_event(name)` | `silcrow-trigger` | Fire DOM CustomEvent |
| `.patch_target(sel, &data)` | `silcrow-patch` | Patch secondary element |
| `.invalidate_target(sel)` | `silcrow-invalidate` | Rebuild binding maps |
| `.client_navigate(path)` | `silcrow-navigate` | Follow-up navigation |
| `.sse(route)` | `silcrow-sse` | Open SSE connection |
| `.ws(route)` | `silcrow-ws` | Open WebSocket connection |

## Toast: `.with_toast(msg, level)`

Shows a notification on the client. Transport depends on response type:

**HTML / Navigate responses** — set as a short-lived cookie:

```rust
html("<p>Saved</p>").with_toast("Saved!", "success")
navigate("/items").with_toast("Redirected", "info")
```

The cookie (`silcrow_toasts`, `Max-Age=5`, `SameSite=Lax`) persists across redirects. Silcrow.js reads and deletes it on the next page load.

**JSON responses** — injected into the payload:

```rust
json(&user).with_toast("Loaded", "info")
// Result: {"name": "Alice", "_toasts": [{"message": "Loaded", "level": "info"}]}
```

If the JSON root is an array, Pilcrow wraps it:

```rust
json(&vec![1, 2, 3]).with_toast("Done", "info")
// Result: {"data": [1, 2, 3], "_toasts": [{"message": "Done", "level": "info"}]}
```

**Multiple toasts** — chain calls:

```rust
html("<p>Done</p>")
    .with_toast("3 items created", "success")
    .with_toast("1 skipped (duplicate)", "warning")
```

**Shared toast** — apply to whichever arm runs:

```rust
respond!(req, {
    html => html(markup),
    json => json(&data),
    toast => ("Saved!", "success"),
})
```

### Toast Levels

Standard levels (used for CSS styling on the client):

| Level | Use |
| --- | --- |
| `"success"` | Completed actions |
| `"info"` | Informational messages |
| `"warning"` | Non-blocking issues |
| `"error"` | Failed operations |

## Custom Headers: `.with_header(key, value)`

Set arbitrary response headers:

```rust
html("<p>test</p>")
    .with_header("x-request-id", format!("req-{}", id))
    .with_header("x-api-version", "v2")
```

## No Cache: `.no_cache()`

Prevents Silcrow.js from caching this response:

```rust
html("<p>Real-time data</p>").no_cache()
```

Sets `silcrow-cache: no-cache`. Without this, Silcrow.js caches GET responses for 5 minutes client-side.

## Retarget: `.retarget(selector)`

Override where the content gets swapped:

```rust
html("<p>Sidebar content</p>").retarget("#sidebar")
```

The client originally targeted one element, but the server response redirects the swap to `#sidebar` instead.

## Push History: `.push_history(url)`

Override the URL shown in the browser address bar:

```rust
html(render_item(&item))
    .push_history(&format!("/items/{}", item.id))
```

The response content is swapped normally, but the browser URL changes to `/items/42`.

## Trigger Event: `.trigger_event(name)`

Fire a custom DOM event on the client:

```rust
html("<p>Updated</p>").trigger_event("refresh-sidebar")
```

On the client, this dispatches `new CustomEvent("refresh-sidebar")`. Useful for coordinating between independent components.

## Patch Target: `.patch_target(selector, &data)`

Patch JSON data into a secondary element after the main swap:

```rust
let count = serde_json::json!({"count": 42});
html(render_item(&item))
    .patch_target("#item-count", &count)
```

The `#item-count` element's `s-bind="count"` attribute gets updated to `42`.

## Invalidate Target: `.invalidate_target(selector)`

Rebuild Silcrow.js binding maps for a subtree:

```rust
html("<p>Structure changed</p>")
    .invalidate_target("#form")
```

Use when HTML structure has changed (new `s-bind` attributes) and bindings need re-scanning.

## Client Navigate: `.client_navigate(path)`

Trigger a follow-up client-side navigation after the swap:

```rust
html("<p>Processing...</p>")
    .client_navigate("/dashboard")
```

The main content is swapped first, then Silcrow.js navigates to `/dashboard`.

## SSE: `.sse(route)`

Signal the client to open an SSE connection:

```rust
html(markup).sse(DASH_EVENTS) // DASH_EVENTS: SseRoute
```

See the [SSE Guide](sse-guide.md) for the full walkthrough.

## WebSocket: `.ws(route)`

Signal the client to open a WebSocket connection:

```rust
html(markup).ws(CHAT_WS) // CHAT_WS: WsRoute
```

See the [WebSocket Guide](ws-guide.md) for the full walkthrough.

## Execution Order

When multiple side effects are present, they execute in this order after the main swap:

```text
1. patch_target    — update secondary elements
2. invalidate_target — rebuild bindings
3. client_navigate — follow-up navigation
4. sse / ws        — open live connections
```

## Chaining

All modifiers return `Self`, so chain freely:

```rust
html(markup)
    .with_toast("Saved!", "success")
    .no_cache()
    .retarget("#main")
    .patch_target("#counter", &count_data)
    .invalidate_target("#sidebar")
    .push_history("/items/42")
    .trigger_event("item-saved")
    .with_header("x-request-id", request_id)
```
