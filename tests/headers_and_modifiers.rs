// tests/headers_and_modifiers.rs
//
// Verify every ResponseExt modifier sets the correct header.

use axum::response::{IntoResponse, Response};
use pilcrow::{html, json, response::ResponseExt, SseRoute, WsRoute};

// ── Helpers ─────────────────────────────────────────────────

fn get_header(response: &Response, name: &str) -> Option<String> {
    response
        .headers()
        .get(name)
        .map(|v| v.to_str().unwrap().to_string())
}

// ════════════════════════════════════════════════════════════
// Custom Headers
// ════════════════════════════════════════════════════════════

#[tokio::test]
async fn with_header_static_value() {
    let response = html("<p>test</p>")
        .with_header("x-custom", "hello")
        .into_response();
    assert_eq!(get_header(&response, "x-custom").unwrap(), "hello");
}

#[tokio::test]
async fn with_header_dynamic_value() {
    let id = 42;
    let response = html("<p>test</p>")
        .with_header("x-request-id", format!("req-{id}"))
        .into_response();
    assert_eq!(get_header(&response, "x-request-id").unwrap(), "req-42");
}

#[tokio::test]
async fn with_header_on_json() {
    let response = json(serde_json::json!({}))
        .with_header("x-api-version", "v2")
        .into_response();
    assert_eq!(get_header(&response, "x-api-version").unwrap(), "v2");
}

// ════════════════════════════════════════════════════════════
// No Cache
// ════════════════════════════════════════════════════════════

#[tokio::test]
async fn no_cache_sets_header() {
    let response = html("<p>test</p>").no_cache().into_response();
    assert_eq!(get_header(&response, "silcrow-cache").unwrap(), "no-cache");
}

#[tokio::test]
async fn no_cache_on_json() {
    let response = json(serde_json::json!({})).no_cache().into_response();
    assert_eq!(get_header(&response, "silcrow-cache").unwrap(), "no-cache");
}

// ════════════════════════════════════════════════════════════
// Retarget
// ════════════════════════════════════════════════════════════

#[tokio::test]
async fn retarget_sets_header() {
    let response = html("<p>test</p>").retarget("#sidebar").into_response();
    assert_eq!(
        get_header(&response, "silcrow-retarget").unwrap(),
        "#sidebar"
    );
}

// ════════════════════════════════════════════════════════════
// Push History
// ════════════════════════════════════════════════════════════

#[tokio::test]
async fn push_history_sets_header() {
    let response = html("<p>test</p>")
        .push_history("/custom-url")
        .into_response();
    assert_eq!(
        get_header(&response, "silcrow-push").unwrap(),
        "/custom-url"
    );
}

// ════════════════════════════════════════════════════════════
// Trigger Event
// ════════════════════════════════════════════════════════════

#[tokio::test]
async fn trigger_event_sets_header() {
    let response = html("<p>test</p>").trigger_event("refresh").into_response();
    let header = get_header(&response, "silcrow-trigger").unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&header).unwrap();
    assert!(parsed.get("refresh").is_some());
}

// ════════════════════════════════════════════════════════════
// Patch Target
// ════════════════════════════════════════════════════════════

#[tokio::test]
async fn patch_target_sets_header() {
    let count = serde_json::json!({"count": 42});
    let response = html("<p>test</p>")
        .patch_target("#counter", &count)
        .into_response();
    let header = get_header(&response, "silcrow-patch").unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&header).unwrap();
    assert_eq!(parsed["target"], "#counter");
    assert_eq!(parsed["data"]["count"], 42);
}

// ════════════════════════════════════════════════════════════
// Invalidate Target
// ════════════════════════════════════════════════════════════

#[tokio::test]
async fn invalidate_target_sets_header() {
    let response = html("<p>test</p>")
        .invalidate_target("#form")
        .into_response();
    assert_eq!(
        get_header(&response, "silcrow-invalidate").unwrap(),
        "#form"
    );
}

// ════════════════════════════════════════════════════════════
// Client Navigate
// ════════════════════════════════════════════════════════════

#[tokio::test]
async fn client_navigate_sets_header() {
    let response = html("<p>test</p>").client_navigate("/next").into_response();
    assert_eq!(get_header(&response, "silcrow-navigate").unwrap(), "/next");
}

// ════════════════════════════════════════════════════════════
// SSE Header
// ════════════════════════════════════════════════════════════

#[tokio::test]
async fn sse_sets_header() {
    const EVENTS: SseRoute = SseRoute::new("/events/feed");
    let response = html("<p>test</p>").sse(EVENTS).into_response();
    assert_eq!(
        get_header(&response, "silcrow-sse").unwrap(),
        "/events/feed"
    );
}

// ════════════════════════════════════════════════════════════
// WS Header
// ════════════════════════════════════════════════════════════

#[tokio::test]
async fn ws_sets_header() {
    const CHAT: WsRoute = WsRoute::new("/ws/chat");
    let response = html("<p>test</p>").ws(CHAT).into_response();
    assert_eq!(get_header(&response, "silcrow-ws").unwrap(), "/ws/chat");
}

// ════════════════════════════════════════════════════════════
// Chained Modifiers
// ════════════════════════════════════════════════════════════

#[tokio::test]
async fn chained_modifiers_all_present() {
    let response = html("<p>test</p>")
        .with_header("x-custom", "value")
        .no_cache()
        .retarget("#main")
        .push_history("/final")
        .trigger_event("done")
        .with_toast("Complete", "success")
        .into_response();

    assert_eq!(get_header(&response, "x-custom").unwrap(), "value");
    assert_eq!(get_header(&response, "silcrow-cache").unwrap(), "no-cache");
    assert_eq!(get_header(&response, "silcrow-retarget").unwrap(), "#main");
    assert_eq!(get_header(&response, "silcrow-push").unwrap(), "/final");
    assert!(get_header(&response, "silcrow-trigger").is_some());

    // Toast via cookie
    let cookies: Vec<_> = response
        .headers()
        .get_all(axum::http::header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().unwrap().to_string())
        .collect();
    assert!(cookies.iter().any(|c| c.starts_with("silcrow_toasts=")));
}
