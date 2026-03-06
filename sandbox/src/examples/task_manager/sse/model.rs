use crate::examples::task_manager::standard::model::TaskStats;

#[derive(Clone)]
pub struct AppStateSse {
    pub tx: tokio::sync::broadcast::Sender<TaskStats>,
}

impl AppStateSse {
    pub fn new() -> Self {
        let (tx, _) = tokio::sync::broadcast::channel(100);
        Self { tx }
    }
}
