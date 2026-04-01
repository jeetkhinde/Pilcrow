use axum::http::HeaderMap;

pub fn is_authenticated(headers: &HeaderMap) -> bool {
    headers
        .get("x-user-id")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| !v.trim().is_empty())
}
