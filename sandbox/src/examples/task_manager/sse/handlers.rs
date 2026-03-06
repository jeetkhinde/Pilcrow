use async_stream::stream;
use axum::{extract::Extension, response::IntoResponse};
use std::convert::Infallible;

use super::model::AppStateSse;
use crate::examples::task_manager::standard::model::{AppState, TaskStats};

pub async fn task_events(
    Extension(state): Extension<AppState>,
    Extension(sse_state): Extension<AppStateSse>,
) -> impl IntoResponse {
    let mut rx = sse_state.tx.subscribe();
    let stream = stream! {
        let current_stats = {
            let tasks = state.tasks.lock().unwrap();
            TaskStats::from(&tasks)
        };
        yield Ok::<_, Infallible>(
            pilcrow::SilcrowEvent::patch(&current_stats, "#live-stats").into()
        );
        while let Ok(stats) = rx.recv().await {
            yield Ok::<_, Infallible>(
                pilcrow::SilcrowEvent::patch(&stats, "#live-stats").into()
            );
        }
    };
    pilcrow::sse(stream)
}
