// examples/hello.rs
//
// Minimal "Hello, World" — the simplest possible Pilcrow handler.
//
// Run:  cargo run --example hello
// Test: curl http://127.0.0.1:3000
//       curl -H "Accept: application/json" http://127.0.0.1:3000

use axum::{response::Response, routing::get, Router};
use pilcrow::*;
use serde::Serialize;

// ── Data ────────────────────────────────────────────────────

#[derive(Serialize)]
struct Greeting {
    message: String,
}

// ── Handler ─────────────────────────────────────────────────

async fn home(req: SilcrowRequest) -> Result<Response, Response> {
    let greeting = Greeting {
        message: "Hello from Pilcrow!".into(),
    };

    // Full-page HTML for browsers, JSON for API clients
    respond!(req, {
        html => html(format!(
            r#"<!DOCTYPE html>
<html>
<head><title>Hello</title>{}</head>
<body>
  <h1>{}</h1>
  <p>Try: <code>curl -H "Accept: application/json" http://127.0.0.1:3000</code></p>
</body>
</html>"#,
            assets::script_tag(),
            greeting.message
        )),
        json => raw greeting,
    })
}

// ── Main ────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route(&assets::silcrow_js_path(), get(assets::serve_silcrow_js))
        .route("/", get(home));

    println!("Listening on http://127.0.0.1:3000");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}
