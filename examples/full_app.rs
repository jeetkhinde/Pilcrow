// examples/full_app.rs
//
// Production-realistic multi-route application combining all features.
//
// Run:  cargo run --example full_app
//
// Routes:
//   GET  /             — Home (dual-mode)
//   GET  /items        — List (dual-mode)
//   GET  /items/:id    — Detail (dual-mode with modifiers)
//   POST /items        — Create (redirect + toast + cache bust)
//   GET  /dashboard    — Dashboard (SSE live updates)
//   GET  /events/dash  — SSE stream
//   GET  /chat         — Chat page (WebSocket)
//   WS   /ws/chat      — WebSocket endpoint

use axum::{
    extract::ws::WebSocketUpgrade,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use pilcrow::{ws::WsEvent, *};
use serde::Serialize;
use std::convert::Infallible;
use std::time::Duration;

// ── Route Constants ─────────────────────────────────────────

const DASH_EVENTS: SseRoute = SseRoute::new("/events/dash");
const CHAT_WS: WsRoute = WsRoute::new("/ws/chat");

// ── Data Models ─────────────────────────────────────────────

#[derive(Serialize, Clone)]
struct Item {
    id: i64,
    name: String,
    price: f64,
}

#[derive(Serialize)]
struct DashStats {
    total_items: u32,
    revenue: f64,
}

// ── Layout ──────────────────────────────────────────────────

fn layout(title: &str, content: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
  <title>{title} — Pilcrow Demo</title>
  {script}
</head>
<body s-debug>
  <nav>
    <a s-action="/">Home</a> |
    <a s-action="/items">Items</a> |
    <a s-action="/dashboard">Dashboard</a> |
    <a s-action="/chat">Chat</a>
  </nav>
  <main>{content}</main>
</body>
</html>"#,
        title = title,
        script = assets::script_tag(),
        content = content
    )
}

// ── Handlers ────────────────────────────────────────────────

/// Home — full page HTML or JSON greeting
async fn home(req: SilcrowRequest) -> Result<Response, Response> {
    respond!(req, {
        html => html(layout("Home", "<h1>Welcome to Pilcrow</h1><p>A full-featured demo app.</p>")),
        json => json(serde_json::json!({"page": "home", "version": "0.1.0"})),
    })
}

/// Items list — dual-mode
async fn list_items(req: SilcrowRequest) -> Result<Response, Response> {
    let items = vec![
        Item {
            id: 1,
            name: "Widget".into(),
            price: 9.99,
        },
        Item {
            id: 2,
            name: "Gadget".into(),
            price: 24.99,
        },
        Item {
            id: 3,
            name: "Doohickey".into(),
            price: 4.50,
        },
    ];

    let rows: String = items
        .iter()
        .map(|i| {
            format!(
                r#"<tr><td>{}</td><td>{}</td><td>${:.2}</td></tr>"#,
                i.id, i.name, i.price
            )
        })
        .collect();

    let table = format!(
        r#"<h1>Items</h1>
<table><thead><tr><th>ID</th><th>Name</th><th>Price</th></tr></thead>
<tbody>{rows}</tbody></table>
<form s-action="/items" method="POST">
  <input name="name" placeholder="Name" />
  <input name="price" type="number" step="0.01" placeholder="Price" />
  <button type="submit">Create</button>
</form>"#
    );

    respond!(req, {
        html => html(layout("Items", &table)),
        json => raw items,
    })
}

/// Create item — POST handler with redirect + toast
async fn create_item() -> Response {
    // In a real app: parse body, insert into DB
    navigate("/items")
        .with_toast("Item created!", "success")
        .into_response()
}

/// Item detail — partial HTML with modifiers
async fn item_detail(req: SilcrowRequest) -> Result<Response, Response> {
    let item = Item {
        id: 42,
        name: "Premium Widget".into(),
        price: 49.99,
    };

    let count_data = serde_json::json!({"count": 3});

    respond!(req, {
        html => html(format!(
            r#"<div class="item-detail">
  <h2>{}</h2>
  <p>Price: ${:.2}</p>
  <span id="cart-count" s-bind="count">0</span> in cart
</div>"#,
            item.name, item.price
        ))
        .push_history(&format!("/items/{}", item.id))
        .patch_target("#cart-count", &count_data)
        .with_toast("Item loaded", "info"),
        json => json(&item),
    })
}

/// Dashboard — SSE live updates
async fn dashboard(req: SilcrowRequest) -> Result<Response, Response> {
    let stats = DashStats {
        total_items: 42,
        revenue: 1234.56,
    };

    let content = format!(
        r#"<h1>Dashboard</h1>
<div id="dash" s-bind="total_items">{}</div>
<div s-bind="revenue">{:.2}</div>"#,
        stats.total_items, stats.revenue
    );

    respond!(req, {
        html => html(layout("Dashboard", &content)).sse(DASH_EVENTS),
        json => json(&stats),
    })
}

/// SSE stream for dashboard
async fn dash_stream() -> impl IntoResponse {
    let stream = async_stream::stream! {
        let mut tick = 0u64;
        loop {
            let stats = DashStats {
                total_items: 42 + tick as u32,
                revenue: 1234.56 + tick as f64 * 10.0,
            };
            yield Ok::<_, Infallible>(
                SilcrowEvent::patch(&stats, "#dash").into()
            );
            tokio::time::sleep(Duration::from_secs(3)).await;
            tick += 1;
        }
    };
    pilcrow::sse(stream)
}

/// Chat page — WebSocket
async fn chat(req: SilcrowRequest) -> Result<Response, Response> {
    let content = r#"<h1>Chat</h1>
<div id="chat-status" s-bind="status">Connecting...</div>
<div id="messages"></div>"#;

    respond!(req, {
        html => html(layout("Chat", content)).ws(CHAT_WS),
        json => json(serde_json::json!({"ws": CHAT_WS.path()})),
    })
}

/// WebSocket handler
async fn chat_handler(upgrade: WebSocketUpgrade) -> impl IntoResponse {
    ws::ws(upgrade, |mut stream| async move {
        stream
            .send(WsEvent::patch(
                serde_json::json!({"status": "connected"}),
                "#chat-status",
            ))
            .await
            .ok();

        while let Some(Ok(event)) = stream.recv().await {
            if let WsEvent::Custom { data, .. } = event {
                stream
                    .send(WsEvent::html(format!("<p>{}</p>", data), "#messages"))
                    .await
                    .ok();
            }
        }
    })
}

// ── Main ────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route(&assets::silcrow_js_path(), get(assets::serve_silcrow_js))
        .route("/", get(home))
        .route("/items", get(list_items).post(create_item))
        .route("/items/:id", get(item_detail))
        .route("/dashboard", get(dashboard))
        .route(DASH_EVENTS.path(), get(dash_stream))
        .route("/chat", get(chat))
        .route(CHAT_WS.path(), get(chat_handler));

    println!("Listening on http://127.0.0.1:3000");
    println!("  GET  /             — Home");
    println!("  GET  /items        — Item list");
    println!("  POST /items        — Create item (redirect)");
    println!("  GET  /items/:id    — Item detail");
    println!("  GET  /dashboard    — Dashboard (SSE)");
    println!("  GET  /events/dash  — SSE stream");
    println!("  GET  /chat         — Chat (WebSocket)");
    println!("  WS   /ws/chat      — WebSocket endpoint");

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}
