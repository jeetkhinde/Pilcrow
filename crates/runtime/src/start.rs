use axum::Router;
use pilcrow_core::PilcrowConfig;
use std::sync::Arc;

pub async fn start(app: Router) {
    let config = Arc::new(PilcrowConfig::load_from_current_dir().expect("load Pilcrow.toml"));
    let bind_addr = config.web_bind_addr();
    let http = reqwest::Client::new();

    let app = app
        .layer(axum::Extension(config))
        .layer(axum::Extension(http));

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .expect("bind");

    println!("listening on http://{bind_addr}");
    axum::serve(listener, app).await.expect("serve");
}
