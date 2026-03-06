use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub title: String,
    pub completed: bool,
}

#[derive(Clone, Serialize)]
pub struct TaskStats {
    pub tasks: TaskCounts,
}

#[derive(Clone, Serialize)]
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
                    title: "Build Pilcrow SSE".into(),
                    completed: false,
                },
            ])),
            next_id: std::sync::Arc::new(std::sync::Mutex::new(3)),
        }
    }
    // the SSE controller doesn't need its own hardcoded initial state block since it is initialized differently
}

use tokio::sync::broadcast;

/// Represents the minimal payload pushed to `#task-list` SSE subscribers
/// for each kind of mutation.
#[derive(Clone, Serialize)]
#[serde(untagged)]
pub enum TaskListEvent {
    /// A new task was added.
    Added(Task),
    /// A task's `completed` field was toggled.
    Toggled { id: i64, completed: bool },
    /// A task was deleted.
    Removed { id: i64, _remove: bool },
}

#[derive(Clone)]
pub struct AppStateSse {
    /// Broadcasts new `TaskStats` to every connected `#live-stats` SSE stream.
    pub tx: broadcast::Sender<TaskStats>,
    /// Broadcasts task-list mutations to every connected `#task-list` SSE stream.
    pub list_tx: broadcast::Sender<TaskListEvent>,
}

impl AppStateSse {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(100);
        let (list_tx, _) = broadcast::channel(100);
        Self { tx, list_tx }
    }
}
