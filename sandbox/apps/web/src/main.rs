mod api {
    pub mod todos;
}

mod backend_client;
mod contracts;

mod generated_api {
    include!(concat!(env!("OUT_DIR"), "/generated_api_routes.rs"));
}

mod generated_routes {
    include!(concat!(env!("OUT_DIR"), "/generated_routes.rs"));
}

mod generated_templates {
    include!(concat!(env!("OUT_DIR"), "/generated_templates.rs"));
}

use axum::{
    Form, Router,
    extract::State,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use backend_client::{RestTodosClient, TodosApi};
use contracts::TodoDto;
use pilcrow_web::{
    PilcrowConfig, ResponseExt, SilcrowEvent, StatusCode, ToastLevel, navigate, sse_stream,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub(crate) struct AppState {
    todos_api: Arc<dyn TodosApi>,
}

#[derive(Debug, Deserialize)]
struct CreateTodoForm {
    title: String,
}

#[derive(Debug, Clone, Serialize)]
struct CounterPayload {
    count: u64,
}

#[tokio::main]
async fn main() {
    let config = PilcrowConfig::load_from_current_dir().expect("load Pilcrow.toml");
    let backend_base_url = config.web.backend_url.clone();
    let bind_addr = config.web_bind_addr();

    let state = AppState {
        todos_api: Arc::new(RestTodosClient::new(backend_base_url)),
    };

    let api_router =
        generated_api::register_generated_api_routes(Router::new(), |router, route| {
            match route.pattern {
                "/api/todos" => router.nest(route.pattern, api::todos::router()),
                _ => router,
            }
        });

    let page_router =
        generated_routes::register_generated_routes(Router::new(), |router, route| {
            match route.pattern {
                "/" => router.route(route.pattern, get(index)),
                _ => router,
            }
        });

    let app = page_router
        .route("/todos", post(create_todo))
        .route("/events/counter", get(counter_events))
        .merge(api_router)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .expect("bind web server");
    println!("web listening on http://{bind_addr}");
    axum::serve(listener, app).await.expect("serve web");
}

async fn index(State(state): State<AppState>) -> Response {
    match state.todos_api.list_todos().await {
        Ok(items) => render_index(items),
        Err(err) => (
            StatusCode::BAD_GATEWAY,
            Html(format!("backend call failed: {err}")),
        )
            .into_response(),
    }
}

async fn create_todo(State(state): State<AppState>, Form(form): Form<CreateTodoForm>) -> Response {
    match state.todos_api.create_todo(form.title).await {
        Ok(_) => navigate("/")
            .with_toast("todo created", ToastLevel::Success)
            .retarget("#todo-list")
            .push_history("/")
            .into_response(),
        Err(err) => (
            StatusCode::BAD_GATEWAY,
            Html(format!("backend call failed: {err}")),
        )
            .into_response(),
    }
}

async fn counter_events() -> impl IntoResponse {
    sse_stream(|emit| async move {
        let mut count = 0_u64;
        loop {
            count += 1;
            emit.send(SilcrowEvent::json(
                CounterPayload { count },
                "#live-counter",
            ))
            .await?;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    })
}

fn render_index(items: Vec<TodoDto>) -> Response {
    let todos = items
        .into_iter()
        .map(|todo| generated_templates::page_index::TodoView {
            id: todo.id,
            title: todo.title,
            done: todo.done,
        })
        .collect::<Vec<_>>();

    let props = generated_templates::page_index::Props {
        title: "Pilcrow Sandbox".to_string(),
        todos,
        sse_path: "/events/counter".to_string(),
    };

    match generated_templates::page_index::render_page_index(props) {
        Ok(markup) => Html(markup).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!("template error: {err}")),
        )
            .into_response(),
    }
}
