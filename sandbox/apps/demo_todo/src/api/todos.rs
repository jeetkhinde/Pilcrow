use pilcrow_macros::handler;
use pilcrow_web::{ResponseExt, ToastLevel};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct CreateTodo {
    pub title: String,
}

#[derive(Deserialize, Serialize)]
pub struct Todo {
    pub id: i64,
    pub title: String,
    pub done: bool,
}

#[handler]
pub async fn create(form: CreateTodo) {
    let todo: Todo = client.post("/api/todos", form).await?;
    pilcrow_web::json(todo).with_toast("Todo created!", ToastLevel::Success)
}

pub fn router() -> axum::Router {
    axum::Router::new().route("/", axum::routing::post(create))
}