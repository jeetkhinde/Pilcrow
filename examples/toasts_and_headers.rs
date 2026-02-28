// examples/toasts_and_headers.rs
//
// Toast transport and custom headers.
//
// Run:  cargo run --example toasts_and_headers
// Test:
//   curl -v http://127.0.0.1:3000/shared-toast
//   curl -v -H "Accept: application/json" http://127.0.0.1:3000/json-toast
//   curl -v http://127.0.0.1:3000/custom-headers

use axum::{response::Response, routing::get, Router};
use pilcrow::*;
use serde::Serialize;

#[derive(Serialize)]
struct Item {
    id: i64,
    name: String,
}

// ── Handlers ────────────────────────────────────────────────

/// Shared toast — applied to whichever branch runs
async fn shared_toast(req: SilcrowRequest) -> Result<Response, Response> {
    respond!(req, {
        html => html("<p>Item saved successfully</p>"),
        json => json(&Item { id: 1, name: "Widget".into() }),
        toast => ("Saved!", "success"),
    })
}

/// Per-arm toasts — different message per format
async fn per_arm_toast(req: SilcrowRequest) -> Result<Response, Response> {
    respond!(req, {
        html => html("<p>Dashboard</p>")
            .with_toast("Welcome back!", "info"),
        json => json(serde_json::json!({"status": "ok"}))
            .with_toast("API access logged", "info"),
    })
}

/// Multiple toasts on a single response
async fn multiple_toasts(req: SilcrowRequest) -> Result<Response, Response> {
    respond!(req, {
        html => html("<p>Bulk operation complete</p>")
            .with_toast("3 items created", "success")
            .with_toast("1 item skipped (duplicate)", "warning"),
        json => json(serde_json::json!({"created": 3, "skipped": 1})),
    })
}

/// JSON toast wrapping — array payload gets wrapped in {data: [...], _toasts: [...]}
async fn json_array_toast(req: SilcrowRequest) -> Result<Response, Response> {
    let items = vec![
        Item {
            id: 1,
            name: "A".into(),
        },
        Item {
            id: 2,
            name: "B".into(),
        },
    ];

    respond!(req, {
        json => json(&items),
        toast => ("Loaded 2 items", "info"),
    })
}

/// Custom headers — static and dynamic values
async fn custom_headers(req: SilcrowRequest) -> Result<Response, Response> {
    let request_id = format!("req-{}", 42);

    respond!(req, {
        html => html("<p>Custom headers set</p>")
            .with_header("x-custom-static", "hello")
            .with_header("x-request-id", request_id),
        json => json(serde_json::json!({"headers": "set"}))
            .with_header("x-custom-static", "hello"),
    })
}

/// No-cache — prevents Silcrow.js client caching
async fn no_cache(req: SilcrowRequest) -> Result<Response, Response> {
    respond!(req, {
        html => html("<p>This response is never cached</p>").no_cache(),
        json => json(serde_json::json!({"cached": false})).no_cache(),
    })
}

// ── Main ────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/shared-toast", get(shared_toast))
        .route("/per-arm-toast", get(per_arm_toast))
        .route("/multiple-toasts", get(multiple_toasts))
        .route("/json-array-toast", get(json_array_toast))
        .route("/custom-headers", get(custom_headers))
        .route("/no-cache", get(no_cache));

    println!("Listening on http://127.0.0.1:3000");
    println!("  GET /shared-toast      — shared toast");
    println!("  GET /per-arm-toast     — per-arm different toasts");
    println!("  GET /multiple-toasts   — multiple toasts stacked");
    println!("  GET /json-array-toast  — array + toast wrapping");
    println!("  GET /custom-headers    — static + dynamic headers");
    println!("  GET /no-cache          — silcrow-cache: no-cache");

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}
