# Response Modifiers

Modifiers are applied on HTML responses returned by web handlers.

## Core Methods

| Method | Effect |
| --- | --- |
| `.with_toast(msg, level)` | show notification |
| `.retarget(selector)` | swap into another target |
| `.patch_target(sel, &data)` | patch secondary element |
| `.invalidate_target(sel)` | rebuild binding map |
| `.trigger_event(name)` | emit client event |
| `.push_history(url)` | update browser URL |
| `.no_cache()` | disable client cache |
| `.sse(route)` | open SSE channel |
| `.ws(route)` | open WebSocket channel |

## Example

```rust
let html = generated_templates::page_index::render_page_index(props)?;

pilcrow_web::html(html)
    .with_toast("Saved", "success")
    .patch_target("#todo-count", &serde_json::json!({"count": count}))
    .invalidate_target("#todo-list")
    .into_response()
```
