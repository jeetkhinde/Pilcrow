// ./crates/pilcrow/src/select.rs
use crate::extract::{RequestMode, SilcrowRequest};
use crate::response::{html, json, HtmlResponse, JsonResponse};
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

// ════════════════════════════════════════════════════════════
// 1. The Polymorphic Conversion Traits
// ════════════════════════════════════════════════════════════

pub trait IntoPilcrowHtml<E> {
    fn into_pilcrow_html(self) -> Result<HtmlResponse, E>;
}

impl<E> IntoPilcrowHtml<E> for String {
    fn into_pilcrow_html(self) -> Result<HtmlResponse, E> {
        Ok(html(self))
    }
}

impl<E> IntoPilcrowHtml<E> for HtmlResponse {
    fn into_pilcrow_html(self) -> Result<HtmlResponse, E> {
        Ok(self)
    }
}

impl<R, E> IntoPilcrowHtml<E> for Result<R, E>
where
    R: Into<HtmlResponse>,
{
    fn into_pilcrow_html(self) -> Result<HtmlResponse, E> {
        self.map(Into::into)
    }
}

impl<E> IntoPilcrowHtml<E> for Result<String, E> {
    fn into_pilcrow_html(self) -> Result<HtmlResponse, E> {
        self.map(html)
    }
}

pub trait IntoPilcrowJson<E> {
    fn into_pilcrow_json(self) -> Result<Response, E>;
}

impl<T, E> IntoPilcrowJson<E> for JsonResponse<T>
where
    T: serde::Serialize,
{
    fn into_pilcrow_json(self) -> Result<Response, E> {
        Ok(self.into_response())
    }
}

impl<E> IntoPilcrowJson<E> for serde_json::Value {
    fn into_pilcrow_json(self) -> Result<Response, E> {
        Ok(json(self).into_response())
    }
}

impl<T, E> IntoPilcrowJson<E> for Result<T, E>
where
    T: IntoPilcrowJson<E>,
{
    fn into_pilcrow_json(self) -> Result<Response, E> {
        match self {
            Ok(val) => val.into_pilcrow_json(),
            Err(e) => Err(e),
        }
    }
}

// ════════════════════════════════════════════════════════════
// 2. The Type-Erased Responses Builder
// ════════════════════════════════════════════════════════════

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
    pub fn new() -> Self {
        Self {
            html: None,
            json: None,
        }
    }

    pub fn html<F, T>(mut self, f: F) -> Self
    where
        F: FnOnce() -> T + Send + 'static,
        T: IntoPilcrowHtml<E> + 'static,
        E: 'static,
    {
        self.html = Some(Box::new(|| {
            f().into_pilcrow_html().map(|res| res.into_response())
        }));
        self
    }

    pub fn json<F, T>(mut self, f: F) -> Self
    where
        F: FnOnce() -> T + Send + 'static,
        T: IntoPilcrowJson<E> + 'static,
        E: 'static,
    {
        self.json = Some(Box::new(|| f().into_pilcrow_json()));
        self
    }
}

// ════════════════════════════════════════════════════════════
// 3. The Core Selector Implementation
// ════════════════════════════════════════════════════════════

impl SilcrowRequest {
    pub fn select<E>(&self, responses: Responses<E>) -> Result<Response, E>
    where
        E: IntoResponse,
    {
        match self.preferred_mode() {
            RequestMode::Html => {
                if let Some(f) = responses.html {
                    f()
                } else {
                    Ok((StatusCode::NOT_ACCEPTABLE, "HTML not provided").into_response())
                }
            }
            RequestMode::Json => {
                if let Some(f) = responses.json {
                    f()
                } else {
                    Ok((StatusCode::NOT_ACCEPTABLE, "JSON not provided").into_response())
                }
            }
        }
    }
}
