use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use pilcrow_contracts::ApiErrorBody;
use pilcrow_core::AppError;

pub fn app_error_to_response(err: AppError) -> Response {
    let (status, code, message) = match err {
        AppError::NotFound(message) => (StatusCode::NOT_FOUND, "NOT_FOUND", message),
        AppError::Unauthorized => (
            StatusCode::UNAUTHORIZED,
            "UNAUTHORIZED",
            "unauthorized".to_string(),
        ),
        AppError::Validation(message) => (StatusCode::BAD_REQUEST, "VALIDATION", message),
        AppError::Internal => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "internal server error".to_string(),
        ),
    };

    (status, Json(ApiErrorBody { code, message })).into_response()
}
