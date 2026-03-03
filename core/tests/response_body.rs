// tests/response_body.rs
//
// Deep response verification — body content, content types, cookies, and toast transport.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use pilcrow::{html, json, navigate, response::ResponseExt};

// ── Helpers ─────────────────────────────────────────────────

async fn body_bytes(response: Response) -> Vec<u8> {
    use axum::body::to_bytes;
    to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec()
}

async fn body_string(response: Response) -> String {
    String::from_utf8(body_bytes(response).await).unwrap()
}

fn get_header(response: &Response, name: &str) -> Option<String> {
    response
        .headers()
        .get(name)
        .map(|v| v.to_str().unwrap().to_string())
}

fn get_cookies(response: &Response) -> Vec<String> {
    response
        .headers()
        .get_all(axum::http::header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().unwrap().to_string())
        .collect()
}

// ════════════════════════════════════════════════════════════
// HTML Response Body
// ════════════════════════════════════════════════════════════

#[tokio::test]
async fn html_response_has_correct_content_type() {
    let response = html("<h1>Hello</h1>").into_response();
    let ct = get_header(&response, "content-type").unwrap();
    assert!(ct.contains("text/html"), "Expected text/html, got: {ct}");
}

#[tokio::test]
async fn html_response_body_matches() {
    let markup = "<h1>Hello World</h1>";
    let response = html(markup).into_response();
    let body = body_string(response).await;
    assert_eq!(body, markup);
}

#[tokio::test]
async fn html_response_with_dynamic_content() {
    let name = "Jagjeet";
    let markup = format!("<h1>Hello, {name}</h1>");
    let response = html(markup.clone()).into_response();
    let body = body_string(response).await;
    assert_eq!(body, markup);
}

// ════════════════════════════════════════════════════════════
// JSON Response Body
// ════════════════════════════════════════════════════════════

#[tokio::test]
async fn json_response_has_correct_content_type() {
    let response = json(serde_json::json!({"ok": true})).into_response();
    let ct = get_header(&response, "content-type").unwrap();
    assert!(
        ct.contains("application/json"),
        "Expected application/json, got: {ct}"
    );
}

#[tokio::test]
async fn json_response_body_matches() {
    let data = serde_json::json!({"name": "Alice", "age": 30});
    let response = json(&data).into_response();
    let body = body_string(response).await;
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(parsed["name"], "Alice");
    assert_eq!(parsed["age"], 30);
}

#[tokio::test]
async fn json_complex_nested_struct() {
    #[derive(serde::Serialize)]
    struct Inner {
        x: i32,
    }
    #[derive(serde::Serialize)]
    struct Outer {
        name: String,
        inner: Inner,
        tags: Vec<String>,
    }

    let data = Outer {
        name: "test".into(),
        inner: Inner { x: 42 },
        tags: vec!["a".into(), "b".into()],
    };

    let response = json(&data).into_response();
    let body = body_string(response).await;
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(parsed["name"], "test");
    assert_eq!(parsed["inner"]["x"], 42);
    assert_eq!(parsed["tags"][0], "a");
    assert_eq!(parsed["tags"][1], "b");
}

// ════════════════════════════════════════════════════════════
// JSON Toast Injection
// ════════════════════════════════════════════════════════════

#[tokio::test]
async fn json_toast_injected_into_object() {
    let response = json(serde_json::json!({"ok": true}))
        .with_toast("Saved", "success")
        .into_response();
    let body = body_string(response).await;
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(parsed["ok"], true);
    let toasts = parsed["_toasts"].as_array().unwrap();
    assert_eq!(toasts.len(), 1);
    assert_eq!(toasts[0]["message"], "Saved");
    assert_eq!(toasts[0]["level"], "success");
}

#[tokio::test]
async fn json_toast_wraps_non_object() {
    let response = json(serde_json::json!([1, 2, 3]))
        .with_toast("Loaded", "info")
        .into_response();
    let body = body_string(response).await;
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(parsed["data"], serde_json::json!([1, 2, 3]));
    assert!(parsed["_toasts"].is_array());
}

#[tokio::test]
async fn json_multiple_toasts() {
    let response = json(serde_json::json!({}))
        .with_toast("First", "info")
        .with_toast("Second", "warning")
        .with_toast("Third", "error")
        .into_response();
    let body = body_string(response).await;
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    let toasts = parsed["_toasts"].as_array().unwrap();
    assert_eq!(toasts.len(), 3);
    assert_eq!(toasts[0]["message"], "First");
    assert_eq!(toasts[1]["message"], "Second");
    assert_eq!(toasts[2]["message"], "Third");
}

// ════════════════════════════════════════════════════════════
// HTML Toast Cookie
// ════════════════════════════════════════════════════════════

#[tokio::test]
async fn html_toast_sets_cookie() {
    let response = html("<p>Done</p>")
        .with_toast("Saved", "success")
        .into_response();
    let cookies = get_cookies(&response);
    assert!(
        cookies.iter().any(|c| c.starts_with("silcrow_toasts=")),
        "Expected silcrow_toasts cookie, got: {:?}",
        cookies
    );
}

#[tokio::test]
async fn html_toast_cookie_decodes_to_valid_json() {
    let response = html("<p>Done</p>")
        .with_toast("Hello", "info")
        .into_response();
    let cookies = get_cookies(&response);
    let toast_cookie = cookies
        .iter()
        .find(|c| c.starts_with("silcrow_toasts="))
        .unwrap();

    // Extract the value between = and ;
    let value_part = toast_cookie
        .split('=')
        .nth(1)
        .unwrap()
        .split(';')
        .next()
        .unwrap();

    let decoded = urlencoding::decode(value_part).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&decoded).unwrap();
    assert!(parsed.is_array());
    assert_eq!(parsed[0]["message"], "Hello");
}

// ════════════════════════════════════════════════════════════
// Navigate Response
// ════════════════════════════════════════════════════════════

#[tokio::test]
async fn navigate_returns_303() {
    let response = navigate("/login").into_response();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn navigate_sets_location_header() {
    let response = navigate("/dashboard").into_response();
    let location = get_header(&response, "location").unwrap();
    assert_eq!(location, "/dashboard");
}

#[tokio::test]
async fn navigate_toast_via_cookie() {
    let response = navigate("/home")
        .with_toast("Redirected", "info")
        .into_response();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let cookies = get_cookies(&response);
    assert!(cookies.iter().any(|c| c.starts_with("silcrow_toasts=")));
}
