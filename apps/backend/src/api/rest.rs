use axum::{Json, Router, extract::State, routing::get};
use pilcrow_contracts::{CreateTodoRequest, ListTodosResponse, TodoDto};

use crate::{AppState, auth, middleware};

pub fn router() -> Router<AppState> {
    Router::new().route("/api/todos", get(list_todos).post(create_todo))
}

async fn list_todos(
    State(state): State<AppState>,
) -> Result<Json<ListTodosResponse>, axum::response::Response> {
    let items = state
        .service
        .list_todos()
        .await
        .map_err(middleware::app_error_to_response)?;
    Ok(Json(ListTodosResponse { items }))
}

async fn create_todo(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(input): Json<CreateTodoRequest>,
) -> Result<Json<TodoDto>, axum::response::Response> {
    if !auth::is_authenticated(&headers) {
        return Err(middleware::app_error_to_response(
            pilcrow_core::AppError::Unauthorized,
        ));
    }

    let todo = state
        .service
        .create_todo(input.title)
        .await
        .map_err(middleware::app_error_to_response)?;
    Ok(Json(todo))
}
