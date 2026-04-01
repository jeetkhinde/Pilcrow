// tests/sse_events.rs
//
// SSE event serialization and route verification.

use axum::response::sse::Event;
use pilcrow::{SilcrowEvent, SseRoute};

// ════════════════════════════════════════════════════════════
// SseRoute
// ════════════════════════════════════════════════════════════

#[test]
fn sse_route_new_and_path() {
    const FEED: SseRoute = SseRoute::new("/events/feed");
    assert_eq!(FEED.path(), "/events/feed");
}

#[test]
fn sse_route_deref() {
    const FEED: SseRoute = SseRoute::new("/events/feed");
    // Deref allows string comparison
    assert_eq!(&*FEED, "/events/feed");
}

#[test]
fn sse_route_as_ref() {
    const FEED: SseRoute = SseRoute::new("/events/feed");
    let s: &str = FEED.as_ref();
    assert_eq!(s, "/events/feed");
}

#[test]
fn sse_route_equality() {
    const A: SseRoute = SseRoute::new("/events/a");
    const B: SseRoute = SseRoute::new("/events/a");
    const C: SseRoute = SseRoute::new("/events/c");
    assert_eq!(A, B);
    assert_ne!(A, C);
}

// ════════════════════════════════════════════════════════════
// SilcrowEvent::patch
// ════════════════════════════════════════════════════════════

#[test]
fn patch_event_creates_successfully() {
    let _event = SilcrowEvent::patch(serde_json::json!({"count": 42}), "#stats");
}

#[test]
fn patch_event_converts_to_sse_event() {
    let silcrow_event = SilcrowEvent::patch(serde_json::json!({"count": 42}), "#stats");
    let _sse_event: Event = silcrow_event.into();
}

#[test]
fn patch_event_with_complex_data() {
    #[derive(serde::Serialize)]
    struct Stats {
        visitors: u64,
        active: Vec<String>,
        nested: std::collections::HashMap<String, i32>,
    }

    let mut map = std::collections::HashMap::new();
    map.insert("a".into(), 1);
    map.insert("b".into(), 2);

    let data = Stats {
        visitors: 9999,
        active: vec!["alice".into(), "bob".into()],
        nested: map,
    };

    let silcrow_event = SilcrowEvent::patch(&data, "#dashboard");
    let _sse_event: Event = silcrow_event.into();
}

#[test]
fn patch_event_with_empty_data() {
    let event = SilcrowEvent::patch(serde_json::json!({}), "#empty");
    let _sse_event: Event = event.into();
}

// ════════════════════════════════════════════════════════════
// SilcrowEvent::html
// ════════════════════════════════════════════════════════════

#[test]
fn html_event_creates_successfully() {
    let _event = SilcrowEvent::html("<p>Updated</p>", "#content");
}

#[test]
fn html_event_converts_to_sse_event() {
    let silcrow_event = SilcrowEvent::html("<p>Updated</p>", "#content");
    let _sse_event: Event = silcrow_event.into();
}

#[test]
fn html_event_with_complex_markup() {
    let markup = r#"<div class="card">
        <h2>Title</h2>
        <p>Body with <strong>formatting</strong></p>
        <img src="/img.png" alt="test" />
    </div>"#;
    let event = SilcrowEvent::html(markup, "#cards");
    let _sse_event: Event = event.into();
}

#[test]
fn html_event_with_dynamic_content() {
    let name = "Jagjeet";
    let event = SilcrowEvent::html(format!("<p>Hello, {name}</p>"), "#greeting");
    let _sse_event: Event = event.into();
}
