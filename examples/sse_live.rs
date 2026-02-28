// examples/sse_live.rs
//
// Server-Sent Events — live updates with SilcrowEvent.
//
// Run:  cargo run --example sse_live
// Test:
//   curl http://127.0.0.1:3000              (HTML page with SSE header)
//   curl http://127.0.0.1:3000/events/stats (raw SSE stream)

use axum::{response::IntoResponse, response::Response, routing::get, Router};
use pilcrow::*;
use serde::Serialize;
use std::convert::Infallible;
use std::time::Duration;

// ── Route constant ──────────────────────────────────────────

const STATS_EVENTS: SseRoute = SseRoute::new("/events/stats");

// ── Data ────────────────────────────────────────────────────

#[derive(Serialize)]
struct DashboardStats {
    visitors: u64,
    active_users: u32,
    cpu_percent: f64,
}

// ── Handlers ────────────────────────────────────────────────

/// Page handler — tells Silcrow.js to open an SSE connection
async fn dashboard(req: SilcrowRequest) -> Result<Response, Response> {
    let stats = DashboardStats {
        visitors: 1234,
        active_users: 42,
        cpu_percent: 23.5,
    };

    respond!(req, {
        html => html(r#"<html>
<body s-debug>
  <h1>Dashboard</h1>
  <div id="stats" s-bind="visitors">Loading...</div>
  <div s-bind="active_users">-</div>
  <div s-bind="cpu_percent">-</div>
</body>
</html>"#.to_string()).sse(STATS_EVENTS),
        json => json(&stats),
    })
}

/// SSE stream — sends patch events every 2 seconds
async fn stats_stream() -> impl IntoResponse {
    let stream = async_stream::stream! {
        let mut tick = 0u64;
        loop {
            let stats = DashboardStats {
                visitors: 1234 + tick * 10,
                active_users: 42 + (tick % 5) as u32,
                cpu_percent: 23.5 + (tick as f64 * 0.3),
            };

            // Patch event — sends JSON to Silcrow.patch(data, "#stats")
            yield Ok::<_, Infallible>(
                SilcrowEvent::patch(&stats, "#stats").into()
            );

            tokio::time::sleep(Duration::from_secs(2)).await;
            tick += 1;

            // Alternate: send an HTML event every 5 ticks
            if tick % 5 == 0 {
                yield Ok::<_, Infallible>(
                    SilcrowEvent::html(
                        format!("<p>HTML update at tick {tick}</p>"),
                        "#stats"
                    ).into()
                );
            }
        }
    };

    pilcrow::sse(stream)
}

// ── Main ────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(dashboard))
        .route(STATS_EVENTS.path(), get(stats_stream));

    println!("Listening on http://127.0.0.1:3000");
    println!("  GET /              — dashboard page (sets silcrow-sse header)");
    println!("  GET /events/stats  — SSE stream (patch events every 2s)");

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}
