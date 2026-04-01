use askama::Template;
use axum::{
    Form, Router,
    extract::State,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use pilcrow_api_client_rest::{RestTodosClient, TodosApi};
use pilcrow_contracts::TodoDto;
use pilcrow_web::StatusCode;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Template)]
#[template(
    source = r#"<!doctype html>
<html>
  <head><meta charset=\"utf-8\" /><title>Pilcrow Web</title></head>
  <body>
    <h1>Pilcrow Web (BFF)</h1>
    <form action=\"/todos\" method=\"post\">
      <input name=\"title\" placeholder=\"New todo\" />
      <button type=\"submit\">Create</button>
    </form>
    <ul>
      {% for item in items %}
      <li>{% if item.done %}[x]{% else %}[ ]{% endif %} {{ item.title }}</li>
      {% endfor %}
    </ul>
  </body>
</html>"#,
    ext = "html"
)]
struct IndexTemplate {
    items: Vec<TodoDto>,
}

#[derive(Clone)]
struct AppState {
    todos_api: Arc<dyn TodosApi>,
}

#[derive(Debug, Deserialize)]
struct CreateTodoForm {
    title: String,
}

#[tokio::main]
async fn main() {
    let backend_base_url =
        std::env::var("PILCROW_BACKEND_URL").unwrap_or_else(|_| "http://127.0.0.1:4000".into());

    let state = AppState {
        todos_api: Arc::new(RestTodosClient::new(backend_base_url)),
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/todos", post(create_todo))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("bind web server");
    println!("web listening on http://127.0.0.1:3000");
    axum::serve(listener, app).await.expect("serve web");
}

async fn index(State(state): State<AppState>) -> Response {
    match state.todos_api.list_todos().await {
        Ok(items) => match (IndexTemplate { items }).render() {
            Ok(html) => Html(html).into_response(),
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("template error: {err}")),
            )
                .into_response(),
        },
        Err(err) => (
            StatusCode::BAD_GATEWAY,
            Html(format!("backend call failed: {err}")),
        )
            .into_response(),
    }
}

async fn create_todo(State(state): State<AppState>, Form(form): Form<CreateTodoForm>) -> Response {
    if let Err(err) = state.todos_api.create_todo(form.title).await {
        return (
            StatusCode::BAD_GATEWAY,
            Html(format!("backend call failed: {err}")),
        )
            .into_response();
    }
    Redirect::to("/").into_response()
}
