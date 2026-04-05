use axum::{Form, response::Html, Router, extract::State, routing::get};
use pilcrow_contracts::{CreateTodoRequest, ListTodosResponse};
use pilcrow_web::StatusCode;

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(list_todos).post(create_todo))
}

async fn create_todo(
    State(state): State<AppState>,
    // Change Json to Form to seamlessly accept the native HTML form submission
    Form(input): Form<CreateTodoRequest>, 
) -> Result<Html<String>, StatusCode> {
    // 1. Send request to backend BFF
    let todo = state
        .todos_api
        .create_todo(input.title)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    // 2. Render an HTML fragment representing just the new Todo
    // (In a larger app, you would use an Askama template for this snippet)
    let fragment = format!(
        r#"<li data-id="{}">[ ] {}</li>"#,
        todo.id, todo.title
    );

    // 3. Return pure HTML. Silcrow catches this and drops it into `#todo-list`
    Ok(Html(fragment))
}