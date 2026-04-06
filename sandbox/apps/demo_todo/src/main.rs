mod api {
    pub mod todos;
}

mod generated {
    include!(concat!(env!("OUT_DIR"), "/generated_routes.rs"));
    include!(concat!(env!("OUT_DIR"), "/generated_api_routes.rs"));
    include!(concat!(env!("OUT_DIR"), "/generated_templates.rs"));
}

use axum::routing::{get, post};
use pilcrow_web::start;

#[tokio::main]
async fn main() {
    let router = generated::pilcrow_router(
        axum::Router::new(),
        |router, route| match route.pattern {
            "/" => router.route("/", get(|client: PilcrowClient| async move {
    let props = generated::page_index::load(client).await?;
    let html = generated::page_index::render_page_index(props)
        .map_err(|_| pilcrow_web::AppError::Internal)?;
    Ok::<_, pilcrow_web::AppError>(axum::response::Html(html))
})),
            "/api/todos" => router.nest("/api/todos", api::todos::router()),
            _ => router,
        },
    );
    start(router).await
}