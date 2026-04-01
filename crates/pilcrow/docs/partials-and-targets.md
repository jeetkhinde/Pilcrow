# Partials & Targets

Use compiled template fragments in web handlers for targeted updates.

## Rule

- first browser load: return full compiled page HTML
- targeted UI update: return compiled fragment HTML

## Example

```rust
async fn todos_fragment(State(state): State<AppState>) -> Response {
    let todos = match state.todos_api.list_todos().await {
        Ok(items) => items,
        Err(err) => {
            return (StatusCode::BAD_GATEWAY, Html(format!("backend call failed: {err}"))).into_response();
        }
    };

    let props = generated_templates::component_todolist::Props { todos };

    match generated_templates::component_todolist::render_component_todolist(props) {
        Ok(markup) => Html(markup)
            .retarget("#todo-list")
            .into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, Html(err.to_string())).into_response(),
    }
}
```

This stays inside the same single rendering model: compiled templates only.
