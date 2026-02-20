// ./crates/pilcrow/src/select.rs
use crate::extract::{RequestMode, SilcrowRequest};
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

// ════════════════════════════════════════════════════════════
// 1. The Type-Erased Responses Builder
// ════════════════════════════════════════════════════════════

/// Holds the closures for each potential response format.
/// We box the closures to avoid generic trait bound explosions when a user
/// only wants to provide one or two of the formats.
pub struct Responses<E> {
    html: Option<Box<dyn FnOnce() -> Result<Response, E> + Send>>,
    json: Option<Box<dyn FnOnce() -> Result<Response, E> + Send>>,
}

impl<E> Default for Responses<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E> Responses<E> {
    /// Starts an empty set of responses
    pub fn new() -> Self {
        Self {
            html: None,
            json: None,
        }
    }

    /// Registers the HTML response generator.
    pub fn html<F, R>(mut self, f: F) -> Self
    where
        F: FnOnce() -> Result<R, E> + Send + 'static,
        R: IntoResponse + 'static,
        E: 'static,
    {
        // We evaluate the closure, and if it succeeds, convert its output into a standard Axum Response
        self.html = Some(Box::new(|| f().map(|res| res.into_response())));
        self
    }

    /// Registers the JSON response generator.
    pub fn json<F, R>(mut self, f: F) -> Self
    where
        F: FnOnce() -> Result<R, E> + Send + 'static,
        R: IntoResponse + 'static,
        E: 'static,
    {
        self.json = Some(Box::new(|| f().map(|res| res.into_response())));
        self
    }
}

// ════════════════════════════════════════════════════════════
// 2. The Core Selector Implementation
// ════════════════════════════════════════════════════════════

impl SilcrowRequest {
    /// Evaluates the preferred mode and executes *only* the matching closure.
    /// `E` is whatever Error type the developer chooses to use in their app!
    pub fn select<E>(&self, responses: Responses<E>) -> Result<Response, E>
    where
        E: IntoResponse, // The developer's error must be convertible to a Response
    {
        match self.preferred_mode() {
            RequestMode::Html => {
                if let Some(f) = responses.html {
                    f()
                } else {
                    Ok((
                        StatusCode::NOT_ACCEPTABLE,
                        "HTML representation not provided",
                    )
                        .into_response())
                }
            }
            RequestMode::Json => {
                if let Some(f) = responses.json {
                    f()
                } else {
                    Ok((
                        StatusCode::NOT_ACCEPTABLE,
                        "JSON representation not provided",
                    )
                        .into_response())
                }
            }
        }
    }
}
