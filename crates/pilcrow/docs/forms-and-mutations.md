# Forms & Mutations

In the mandatory convention, form handlers live in `apps/web` and call backend REST APIs.

## Example Flow

1. Browser submits form to `apps/web`.
2. Web handler calls backend API client.
3. Web returns redirect or HTML fragment.

## Web Handler Example

```rust
#[derive(Debug, Deserialize)]
struct CreateTodoForm {
    title: String,
}

async fn create_todo(
    State(state): State<AppState>,
    Form(form): Form<CreateTodoForm>,
) -> Response {
    if form.title.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, Html("Title is required")).into_response();
    }

    if let Err(err) = state.todos_api.create_todo(form.title).await {
        return (StatusCode::BAD_GATEWAY, Html(format!("backend call failed: {err}"))).into_response();
    }

    Redirect::to("/").into_response()
}
```

Web does not directly write DB data.
