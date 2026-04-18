use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("unauthorized")]
    Unauthorized,
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("internal server error")]
    Internal,
    /// Redirect to another URL. Use this from `load()` to redirect before rendering.
    ///
    /// ```rust,ignore
    /// pub async fn load(ctx: PageContext) -> AppResult<Props> {
    ///     if !authenticated { return Err(AppError::redirect("/login")); }
    ///     Ok(Props { ... })
    /// }
    /// ```
    #[error("redirect to {0}")]
    Redirect(String),
}

impl AppError {
    /// Convenience constructor for `AppError::Redirect`.
    pub fn redirect(path: impl Into<String>) -> Self {
        AppError::Redirect(path.into())
    }

    /// HTTP status code that best represents this error.
    pub fn status_code(&self) -> u16 {
        match self {
            AppError::NotFound(_) => 404,
            AppError::Unauthorized => 401,
            AppError::Validation(_) => 422,
            AppError::Internal => 500,
            AppError::Redirect(_) => 303,
        }
    }
}
