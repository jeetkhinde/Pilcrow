// ./crates/pilcrow/src/extract.rs

use crate::headers::SilcrowTarget;
use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use headers::HeaderMapExt;

// ════════════════════════════════════════════════════════════
// 1. The Unified Mode Enum
// ════════════════════════════════════════════════════════════
#[derive(Debug, PartialEq, Eq)]
pub enum RequestMode {
    Html,
    Json,
}

// ════════════════════════════════════════════════════════════
// 2. The Extractor Struct
// ════════════════════════════════════════════════════════════
#[derive(Debug, Clone)]
pub struct SilcrowRequest {
    pub is_silcrow: bool,
    pub accepts_html: bool,
    pub accepts_json: bool,
}

#[async_trait]
impl<S> FromRequestParts<S> for SilcrowRequest
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Did silcrow.js send this request?
        let is_silcrow = parts.headers.typed_get::<SilcrowTarget>().is_some();

        // What data format does the client want?
        let accept_header = parts
            .headers
            .get(axum::http::header::ACCEPT)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        let mut max_html_q = 0.0_f32;
        let mut max_json_q = 0.0_f32;

        for part in accept_header.split(',') {
            let mut iter = part.split(';');
            let media_type = iter.next().unwrap_or("").trim();

            let q: f32 = iter
                .find_map(|param| param.trim().strip_prefix("q=").and_then(|v| v.parse().ok()))
                .unwrap_or(1.0);

            if media_type == "text/html" || media_type == "*/*" {
                max_html_q = max_html_q.max(q);
            }
            if media_type == "application/json" || media_type == "*/*" {
                max_json_q = max_json_q.max(q);
            }
        }

        // Only accept HTML if its computed q-value is greater than or equal to JSON's
        // This resolves: `text/html;q=0.9, application/json;q=1.0` correctly picking JSON
        let accepts_html = max_html_q > 0.0 && max_html_q >= max_json_q;
        let accepts_json = max_json_q > 0.0;

        Ok(SilcrowRequest {
            is_silcrow,
            accepts_html,
            accepts_json,
        })
    }
}

// ════════════════════════════════════════════════════════════
// 3. Content Negotiation Logic
// ════════════════════════════════════════════════════════════
impl SilcrowRequest {
    /// Determines the exact format the handler should return based on headers.
    pub fn preferred_mode(&self) -> RequestMode {
        match (self.is_silcrow, self.accepts_html, self.accepts_json) {
            (true, true, _) => RequestMode::Html,
            (true, false, true) => RequestMode::Json,
            (false, true, _) => RequestMode::Html,
            _ => RequestMode::Json,
        }
    }
}
