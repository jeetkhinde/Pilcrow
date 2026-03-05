use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub title: String,
    pub completed: bool,
}

#[derive(Serialize)]
pub struct TaskStats {
    pub tasks: TaskCounts,
}

#[derive(Serialize)]
pub struct TaskCounts {
    pub length: usize,
    pub pending: usize,
    pub completed: usize,
}

impl TaskStats {
    pub fn from(tasks: &[Task]) -> Self {
        Self {
            tasks: TaskCounts {
                length: tasks.len(),
                pending: tasks.iter().filter(|t| !t.completed).count(),
                completed: tasks.iter().filter(|t| t.completed).count(),
            },
        }
    }
}

#[derive(Deserialize)]
pub struct CreateTask {
    pub title: String,
}

#[derive(Clone)]
pub struct AppState {
    pub tasks: std::sync::Arc<std::sync::Mutex<Vec<Task>>>,
    pub next_id: std::sync::Arc<std::sync::Mutex<i64>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            tasks: std::sync::Arc::new(std::sync::Mutex::new(vec![
                Task {
                    id: 1,
                    title: "Learn Axum".into(),
                    completed: true,
                },
                Task {
                    id: 2,
                    title: "Build Pilcrow".into(),
                    completed: false,
                },
            ])),
            next_id: std::sync::Arc::new(std::sync::Mutex::new(3)),
        }
    }
}
