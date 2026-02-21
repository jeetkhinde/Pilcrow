// ./crates/pilcrow/src/extract.rs

use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};

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
        let is_silcrow = parts.headers.contains_key("silcrow-target");

        // What data format does the client want?
        let accept = parts
            .headers
            .get(axum::http::header::ACCEPT)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        let accepts_html = accept.contains("text/html");
        let accepts_json = accept.contains("application/json");

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
        // If it's a Silcrow AJAX request, respect the Accept header strictly
        if self.is_silcrow {
            if self.accepts_html {
                return RequestMode::Html;
            }
            if self.accepts_json {
                return RequestMode::Json;
            }
        }

        // If it's a standard browser hard-refresh, default to HTML
        if self.accepts_html {
            return RequestMode::Html;
        }

        // Ultimate fallback for API clients
        RequestMode::Json
    }
}

#[cfg(test)]
mod tests {
    use super::{RequestMode, SilcrowRequest};

    #[test]
    fn silcrow_prefers_html_when_requested() {
        let req = SilcrowRequest {
            is_silcrow: true,
            accepts_html: true,
            accepts_json: true,
        };

        assert_eq!(req.preferred_mode(), RequestMode::Html);
    }

    #[test]
    fn silcrow_falls_back_to_json_when_html_not_accepted() {
        let req = SilcrowRequest {
            is_silcrow: true,
            accepts_html: false,
            accepts_json: true,
        };

        assert_eq!(req.preferred_mode(), RequestMode::Json);
    }

    #[test]
    fn silcrow_without_known_accept_defaults_to_json() {
        let req = SilcrowRequest {
            is_silcrow: true,
            accepts_html: false,
            accepts_json: false,
        };

        assert_eq!(req.preferred_mode(), RequestMode::Json);
    }

    #[test]
    fn non_silcrow_browser_defaults_to_html() {
        let req = SilcrowRequest {
            is_silcrow: false,
            accepts_html: true,
            accepts_json: false,
        };

        assert_eq!(req.preferred_mode(), RequestMode::Html);
    }

    #[tokio::test]
    async fn from_request_parts_reads_accept_and_silcrow_headers() {
        use axum::extract::FromRequestParts;
        use axum::http::{header::ACCEPT, Request};

        let request = Request::builder()
            .uri("/")
            .header(ACCEPT, "text/html,application/json")
            .header("silcrow-target", "#main")
            .body(())
            .expect("request should build");
        let (mut parts, _) = request.into_parts();

        let extracted = SilcrowRequest::from_request_parts(&mut parts, &())
            .await
            .expect("extractor should succeed");

        assert!(extracted.is_silcrow);
        assert!(extracted.accepts_html);
        assert!(extracted.accepts_json);
    }

    #[test]
    fn non_silcrow_api_client_defaults_to_json() {
        let req = SilcrowRequest {
            is_silcrow: false,
            accepts_html: false,
            accepts_json: false,
        };

        assert_eq!(req.preferred_mode(), RequestMode::Json);
    }
}
