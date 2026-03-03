// examples/navigation.rs
//
// Redirects and server-driven side-effect headers.
//
// Run:  cargo run --example navigation
// Test:
//   curl -v http://127.0.0.1:3000/redirect
//   curl -v http://127.0.0.1:3000/retarget
//   curl -v http://127.0.0.1:3000/side-effects

use axum::{
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use pilcrow::*;

// ── Handlers ────────────────────────────────────────────────

/// Redirect with toast — navigate() returns 303 See Other
async fn redirect() -> Response {
    navigate("/dashboard")
        .with_toast("Redirected!", "info")
        .into_response()
}

/// Dashboard — landing page after redirect
async fn dashboard(req: SilcrowRequest) -> Result<Response, Response> {
    respond!(req, {
        html => html("<h1>Dashboard</h1><p>You were redirected here.</p>"),
    })
}

/// Retarget — swap content into a different DOM element
async fn retarget(req: SilcrowRequest) -> Result<Response, Response> {
    respond!(req, {
        html => html("<p>This goes into #sidebar instead of the main target</p>")
            .retarget("#sidebar"),
        json => json(serde_json::json!({"retargeted": true})),
    })
}

/// Push history — override the browser URL bar
async fn push_url(req: SilcrowRequest) -> Result<Response, Response> {
    respond!(req, {
        html => html("<h1>Item 42</h1>")
            .push_history("/items/42"),
        json => json(serde_json::json!({"id": 42})),
    })
}

/// Trigger event — fire a custom DOM event from the server
async fn trigger(req: SilcrowRequest) -> Result<Response, Response> {
    respond!(req, {
        html => html("<p>Event triggered</p>")
            .trigger_event("refresh-sidebar"),
        json => json(serde_json::json!({"triggered": "refresh-sidebar"})),
    })
}

/// Side effects — combine multiple server-driven actions
async fn side_effects(req: SilcrowRequest) -> Result<Response, Response> {
    let count_data = serde_json::json!({"count": 99});

    respond!(req, {
        html => html("<p>Item updated</p>")
            .patch_target("#item-count", &count_data)
            .invalidate_target("#sidebar")
            .with_toast("Updated!", "success"),
        json => json(serde_json::json!({"status": "updated"})),
    })
}

/// Client navigate — tell Silcrow.js to perform a follow-up navigation
async fn client_nav(req: SilcrowRequest) -> Result<Response, Response> {
    respond!(req, {
        html => html("<p>Processing...</p>")
            .client_navigate("/dashboard"),
        json => json(serde_json::json!({"next": "/dashboard"})),
    })
}

// ── Main ────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/redirect", get(redirect))
        .route("/dashboard", get(dashboard))
        .route("/retarget", get(retarget))
        .route("/push-url", get(push_url))
        .route("/trigger", get(trigger))
        .route("/side-effects", get(side_effects))
        .route("/client-nav", get(client_nav));

    println!("Listening on http://127.0.0.1:3000");
    println!("  GET /redirect      — 303 redirect with toast");
    println!("  GET /retarget      — silcrow-retarget header");
    println!("  GET /push-url      — silcrow-push header");
    println!("  GET /trigger       — silcrow-trigger header");
    println!("  GET /side-effects  — patch + invalidate + toast");
    println!("  GET /client-nav    — silcrow-navigate header");

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}
