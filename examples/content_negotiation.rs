// examples/content_negotiation.rs
//
// All respond! macro variants in one app.
//
// Run:  cargo run --example content_negotiation
// Test:
//   curl http://127.0.0.1:3000/both
//   curl -H "Accept: application/json" http://127.0.0.1:3000/both
//   curl http://127.0.0.1:3000/raw
//   curl -H "Accept: application/json" http://127.0.0.1:3000/html-only
//   curl http://127.0.0.1:3000/json-only

use axum::{response::Response, routing::get, Router};
use pilcrow::*;
use serde::Serialize;

// ── Data models ─────────────────────────────────────────────

#[derive(Serialize)]
struct User {
    id: i64,
    name: String,
    email: String,
    role: Role,
    tags: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum Role {
    Admin,
    Member,
    Guest,
}

#[derive(Serialize)]
struct HealthStatus {
    status: String,
    uptime_secs: u64,
}

// ── Handlers ────────────────────────────────────────────────

/// Both arms — standard dual-mode handler
async fn both(req: SilcrowRequest) -> Result<Response, Response> {
    let user = User {
        id: 1,
        name: "Jagjeet".into(),
        email: "jeet@example.com".into(),
        role: Role::Admin,
        tags: vec!["rust".into(), "axum".into()],
    };

    let markup = format!(
        r#"<div class="user"><h1>{}</h1><p>{}</p></div>"#,
        user.name, user.email
    );

    respond!(req, {
        html => html(markup),
        json => json(&user),
    })
}

/// Raw JSON shorthand — auto-wrapped in json()
async fn raw(req: SilcrowRequest) -> Result<Response, Response> {
    let user = User {
        id: 2,
        name: "Alice".into(),
        email: "alice@example.com".into(),
        role: Role::Member,
        tags: vec!["web".into()],
    };

    respond!(req, {
        html => html("<h1>Alice</h1>"),
        json => raw user,
    })
}

/// HTML-only — JSON requests get 406
async fn html_only(req: SilcrowRequest) -> Result<Response, Response> {
    respond!(req, {
        html => html("<h1>This page is HTML-only</h1>"),
    })
}

/// JSON-only — HTML requests get 406
async fn json_only(req: SilcrowRequest) -> Result<Response, Response> {
    let status = HealthStatus {
        status: "ok".into(),
        uptime_secs: 3600,
    };

    respond!(req, {
        json => json(&status),
    })
}

/// Raw JSON-only — no HTML arm
async fn json_only_raw(req: SilcrowRequest) -> Result<Response, Response> {
    respond!(req, {
        json => raw HealthStatus {
            status: "ok".into(),
            uptime_secs: 7200,
        },
    })
}

/// Per-arm modifiers with chaining
async fn with_modifiers(req: SilcrowRequest) -> Result<Response, Response> {
    let user = User {
        id: 3,
        name: "Bob".into(),
        email: "bob@example.com".into(),
        role: Role::Guest,
        tags: vec![],
    };

    respond!(req, {
        html => html("<h1>Bob</h1>")
            .with_toast("Loaded", "info")
            .no_cache(),
        json => json(&user)
            .with_toast("Fetched", "success"),
    })
}

// ── Main ────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/both", get(both))
        .route("/raw", get(raw))
        .route("/html-only", get(html_only))
        .route("/json-only", get(json_only))
        .route("/json-only-raw", get(json_only_raw))
        .route("/modifiers", get(with_modifiers));

    println!("Listening on http://127.0.0.1:3000");
    println!("  GET /both        — dual-mode");
    println!("  GET /raw         — raw JSON shorthand");
    println!("  GET /html-only   — HTML only (JSON → 406)");
    println!("  GET /json-only   — JSON only (HTML → 406)");
    println!("  GET /modifiers   — per-arm modifiers");

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}
