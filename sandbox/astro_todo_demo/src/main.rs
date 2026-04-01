use axum::{
    Router,
    extract::{Path as AxPath, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
};
use std::sync::Arc;

mod generated_routes {
    include!(concat!(env!("OUT_DIR"), "/generated_routes.rs"));
}

mod generated_templates {
    include!(concat!(env!("OUT_DIR"), "/generated_templates.rs"));
}

#[derive(Debug, Clone)]
struct Todo {
    id: i64,
    title: String,
    done: bool,
}

#[derive(Clone)]
struct AppState {
    todos: Arc<Vec<Todo>>,
}

#[tokio::main]
async fn main() {
    let state = AppState {
        todos: Arc::new(vec![
            Todo {
                id: 1,
                title: "Write architecture doc".into(),
                done: false,
            },
            Todo {
                id: 2,
                title: "Ship Astro-like routing demo".into(),
                done: true,
            },
            Todo {
                id: 3,
                title: "Refine slot props behavior".into(),
                done: false,
            },
        ]),
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/todos/:id", get(todo_detail))
        .route("/todos/completed", get(completed))
        .with_state(state);

    let preferred = "127.0.0.1:3001";
    let listener = match tokio::net::TcpListener::bind(preferred).await {
        Ok(l) => l,
        Err(_) => tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind preview listener"),
    };
    let local_addr = listener.local_addr().expect("listener local addr");

    println!("Astro Todo preview running at http://{local_addr}");
    println!("Compiled routes:");
    for route in generated_routes::generated_routes() {
        println!("  {} -> {}", route.pattern, route.render_symbol);
    }

    axum::serve(listener, app).await.unwrap();
}

async fn index(State(state): State<AppState>) -> Response {
    let todos = state
        .todos
        .iter()
        .map(|todo| generated_templates::page_index::Todo {
            id: todo.id,
            title: todo.title.clone(),
            done: todo.done,
        })
        .collect::<Vec<_>>();

    let props = generated_templates::page_index::Props {
        title: "Astro Todo Preview".to_string(),
        todos,
    };

    render_compiled(generated_templates::page_index::render_page_index(props))
}

async fn todo_detail(AxPath(id): AxPath<i64>, State(state): State<AppState>) -> Response {
    let Some(todo) = state.todos.iter().find(|t| t.id == id) else {
        return (StatusCode::NOT_FOUND, Html(format!("Todo {id} not found"))).into_response();
    };

    let props = generated_templates::page_todos_id::Props {
        id: todo.id,
        title: todo.title.clone(),
        done: todo.done,
    };

    render_compiled(generated_templates::page_todos_id::render_page_todos_id(
        props,
    ))
}

async fn completed(_state: State<AppState>) -> Response {
    let props = generated_templates::page_todos_completed::Props {
        title: "Completed Todos".to_string(),
    };

    render_compiled(generated_templates::page_todos_completed::render_page_todos_completed(props))
}

fn render_compiled(rendered: Result<String, askama::Error>) -> Response {
    match rendered {
        Ok(html) => Html(html).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!("Template render error: {err}")),
        )
            .into_response(),
    }
}
