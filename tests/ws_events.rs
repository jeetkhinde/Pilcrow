// tests/ws_events.rs
//
// WebSocket event serialization, deserialization, and route verification.

use pilcrow::ws::WsEvent;
use pilcrow::WsRoute;

// ════════════════════════════════════════════════════════════
// WsRoute
// ════════════════════════════════════════════════════════════

#[test]
fn ws_route_new_and_path() {
    const CHAT: WsRoute = WsRoute::new("/ws/chat");
    assert_eq!(CHAT.path(), "/ws/chat");
}

#[test]
fn ws_route_deref() {
    const CHAT: WsRoute = WsRoute::new("/ws/chat");
    assert_eq!(&*CHAT, "/ws/chat");
}

#[test]
fn ws_route_as_ref() {
    const CHAT: WsRoute = WsRoute::new("/ws/chat");
    let s: &str = CHAT.as_ref();
    assert_eq!(s, "/ws/chat");
}

#[test]
fn ws_route_equality() {
    const A: WsRoute = WsRoute::new("/ws/a");
    const B: WsRoute = WsRoute::new("/ws/a");
    const C: WsRoute = WsRoute::new("/ws/c");
    assert_eq!(A, B);
    assert_ne!(A, C);
}

// ════════════════════════════════════════════════════════════
// WsEvent::patch serialization
// ════════════════════════════════════════════════════════════

#[test]
fn ws_patch_serialization() {
    let event = WsEvent::patch(serde_json::json!({"count": 42}), "#stats");
    let json = serde_json::to_string(&event).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["type"], "patch");
    assert_eq!(parsed["target"], "#stats");
    assert_eq!(parsed["data"]["count"], 42);
}

#[test]
fn ws_patch_complex_data() {
    #[derive(serde::Serialize)]
    struct User {
        id: i64,
        name: String,
        roles: Vec<String>,
    }

    let user = User {
        id: 1,
        name: "Alice".into(),
        roles: vec!["admin".into(), "user".into()],
    };

    let event = WsEvent::patch(&user, "#user-card");
    let json = serde_json::to_string(&event).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["type"], "patch");
    assert_eq!(parsed["data"]["name"], "Alice");
    assert_eq!(parsed["data"]["roles"][0], "admin");
}

// ════════════════════════════════════════════════════════════
// WsEvent::html serialization
// ════════════════════════════════════════════════════════════

#[test]
fn ws_html_serialization() {
    let event = WsEvent::html("<p>Hello</p>", "#content");
    let json = serde_json::to_string(&event).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["type"], "html");
    assert_eq!(parsed["target"], "#content");
    assert_eq!(parsed["markup"], "<p>Hello</p>");
}

// ════════════════════════════════════════════════════════════
// WsEvent::invalidate serialization
// ════════════════════════════════════════════════════════════

#[test]
fn ws_invalidate_serialization() {
    let event = WsEvent::invalidate("#form");
    let json = serde_json::to_string(&event).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["type"], "invalidate");
    assert_eq!(parsed["target"], "#form");
}

// ════════════════════════════════════════════════════════════
// WsEvent::navigate serialization
// ════════════════════════════════════════════════════════════

#[test]
fn ws_navigate_serialization() {
    let event = WsEvent::navigate("/dashboard");
    let json = serde_json::to_string(&event).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["type"], "navigate");
    assert_eq!(parsed["path"], "/dashboard");
}

// ════════════════════════════════════════════════════════════
// WsEvent::custom serialization
// ════════════════════════════════════════════════════════════

#[test]
fn ws_custom_serialization() {
    let event = WsEvent::custom("refresh", serde_json::json!({"section": "sidebar"}));
    let json = serde_json::to_string(&event).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["type"], "custom");
    assert_eq!(parsed["event"], "refresh");
    assert_eq!(parsed["data"]["section"], "sidebar");
}

#[test]
fn ws_custom_with_string_event() {
    let event_name = String::from("dynamic-event");
    let event = WsEvent::custom(event_name, serde_json::json!(null));
    let json = serde_json::to_string(&event).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["event"], "dynamic-event");
}

// ════════════════════════════════════════════════════════════
// WsEvent roundtrip (serialize → deserialize)
// ════════════════════════════════════════════════════════════

#[test]
fn ws_patch_roundtrip() {
    let original = WsEvent::patch(serde_json::json!({"x": 1}), "#a");
    let json = serde_json::to_string(&original).unwrap();
    let restored: WsEvent = serde_json::from_str(&json).unwrap();

    match restored {
        WsEvent::Patch { target, data } => {
            assert_eq!(target, "#a");
            assert_eq!(data["x"], 1);
        }
        _ => panic!("Expected Patch variant"),
    }
}

#[test]
fn ws_html_roundtrip() {
    let original = WsEvent::html("<b>bold</b>", "#b");
    let json = serde_json::to_string(&original).unwrap();
    let restored: WsEvent = serde_json::from_str(&json).unwrap();

    match restored {
        WsEvent::Html { target, markup } => {
            assert_eq!(target, "#b");
            assert_eq!(markup, "<b>bold</b>");
        }
        _ => panic!("Expected Html variant"),
    }
}

#[test]
fn ws_invalidate_roundtrip() {
    let original = WsEvent::invalidate("#c");
    let json = serde_json::to_string(&original).unwrap();
    let restored: WsEvent = serde_json::from_str(&json).unwrap();

    match restored {
        WsEvent::Invalidate { target } => assert_eq!(target, "#c"),
        _ => panic!("Expected Invalidate variant"),
    }
}

#[test]
fn ws_navigate_roundtrip() {
    let original = WsEvent::navigate("/home");
    let json = serde_json::to_string(&original).unwrap();
    let restored: WsEvent = serde_json::from_str(&json).unwrap();

    match restored {
        WsEvent::Navigate { path } => assert_eq!(path, "/home"),
        _ => panic!("Expected Navigate variant"),
    }
}

#[test]
fn ws_custom_roundtrip() {
    let original = WsEvent::custom("ping", serde_json::json!({"ts": 12345}));
    let json = serde_json::to_string(&original).unwrap();
    let restored: WsEvent = serde_json::from_str(&json).unwrap();

    match restored {
        WsEvent::Custom { event, data } => {
            assert_eq!(event, "ping");
            assert_eq!(data["ts"], 12345);
        }
        _ => panic!("Expected Custom variant"),
    }
}
