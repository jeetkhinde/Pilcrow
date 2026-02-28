# Forms & Mutations

This guide covers POST handlers, form validation, and the redirect + toast pattern.

## HTML Forms with Silcrow.js

Use `s-action` instead of `action` to make forms navigate via Silcrow.js:

```html
<form s-action="/items" method="POST">
  <input name="name" placeholder="Item name" required />
  <input name="price" type="number" step="0.01" placeholder="Price" />
  <button type="submit">Create</button>
</form>
```

Silcrow.js intercepts submit, sends the form data as `FormData`, and processes the response — no full-page reload.

## Server-Side: Parsing Form Data

Use Axum's `Form` extractor for form submissions and `Json` for API clients:

```rust
use axum::Form;
use serde::Deserialize;

#[derive(Deserialize)]
struct CreateItem {
    name: String,
    price: f64,
}

async fn create_item(Form(input): Form<CreateItem>) -> Response {
    let item = db.insert_item(&input.name, input.price).await?;

    // Redirect back to list with a success toast
    navigate("/items")
        .with_toast("Item created!", "success")
        .into_response()
}
```

## The Redirect + Toast Pattern

For mutations (POST, PUT, DELETE), the standard pattern is:

1. Process the mutation
2. Return `navigate("/path")` with a toast
3. Silcrow.js follows the redirect and renders the new page
4. The toast appears automatically (cookie-based for HTML)

```rust
async fn delete_item(Path(id): Path<i64>) -> Response {
    db.delete_item(id).await?;

    navigate("/items")
        .with_toast("Item deleted", "info")
        .into_response()
}
```

This works for both browser and Silcrow.js requests. The toast survives the redirect because it's set as a short-lived cookie (`Max-Age=5`).

## Returning a Partial After Mutation

Sometimes you want to update the page in-place instead of redirecting. Return HTML directly from the POST handler:

```rust
async fn toggle_favorite(
    req: SilcrowRequest,
    Path(id): Path<i64>,
) -> Result<Response, Response> {
    let item = db.toggle_favorite(id).await?;
    let count = db.favorites_count().await?;

    respond!(req, {
        html => html(render_item_card(&item))
            .patch_target("#fav-count", &serde_json::json!({"count": count}))
            .with_toast("Updated!", "success"),
        json => json(&item),
    })
}
```

The HTML arm returns the updated card, patches the counter, and shows a toast — all in one response.

## Validation Errors

Return the form with error messages when validation fails:

```rust
async fn create_item(
    req: SilcrowRequest,
    Form(input): Form<CreateItem>,
) -> Result<Response, Response> {
    // Validate
    let mut errors = Vec::new();
    if input.name.is_empty() {
        errors.push("Name is required");
    }
    if input.price <= 0.0 {
        errors.push("Price must be positive");
    }

    if !errors.is_empty() {
        return respond!(req, {
            html => html(render_form_with_errors(&input, &errors))
                .with_toast("Please fix the errors", "error"),
            json => json(serde_json::json!({
                "errors": errors,
                "input": { "name": input.name, "price": input.price }
            })),
        });
    }

    // Success path
    let item = db.insert_item(&input.name, input.price).await?;

    respond!(req, {
        html => html(render_item_card(&item)),
        json => json(&item),
        toast => ("Item created!", "success"),
    })
}
```

## JSON API: Same Handler, Different Shape

The `respond!` macro makes it natural to serve both HTML forms and JSON APIs from the same handler. API clients send `Accept: application/json` and get structured responses:

```bash
# HTML form submission
curl -X POST http://localhost:3000/items \
  -d "name=Widget&price=9.99"

# JSON API call
curl -X POST http://localhost:3000/items \
  -H "Content-Type: application/json" \
  -H "Accept: application/json" \
  -d '{"name":"Widget","price":9.99}'
```

Both hit the same handler — the form arm renders HTML, the JSON arm returns structured data.

## Cache Busting

Silcrow.js automatically clears its client-side response cache on any non-GET request. This means after a POST/PUT/DELETE, subsequent navigations re-fetch fresh data from the server. No manual cache management needed.

## Next Steps

- [Partials & Targets](partials-and-targets.md) — control where HTML gets swapped
- [Response Modifiers](response-modifiers.md) — all the modifier methods
