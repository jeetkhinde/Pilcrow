use axum::{Json, Router, extract::State, routing::get};
use pilcrow_contracts::{CreateTodoRequest, ListTodosResponse, TodoDto};
use pilcrow_web::StatusCode;

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_todos).post(create_todo))
}

async fn list_todos(State(state): State<AppState>) -> Result<Json<ListTodosResponse>, StatusCode> {
    state
        .todos_api
        .list_todos()
        .await
        .map(|items| Json(ListTodosResponse { items }))
        .map_err(|_| StatusCode::BAD_GATEWAY)
}

async fn create_todo(
    State(state): State<AppState>,
    Json(input): Json<CreateTodoRequest>,
) -> Result<Json<TodoDto>, StatusCode> {
    state
        .todos_api
        .create_todo(input.title)
        .await
        .map(Json)
        .map_err(|_| StatusCode::BAD_GATEWAY)
}
