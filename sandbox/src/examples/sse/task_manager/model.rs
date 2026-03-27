use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, watch};

// ── Domain types ─────────────────────────────────────────────

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
    pub fn from_tasks(tasks: &[Task]) -> Self {
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

// ── Application state ─────────────────────────────────────────

/// Unified application state for the SSE task manager.
///
/// One instance. One set of channels. All mutations go through `mutate()`,
/// which recomputes stats and fans out to every active SSE connection automatically.
#[derive(Clone)]
pub struct AppState {
    pub tasks: Arc<Mutex<Vec<Task>>>,
    pub next_id: Arc<Mutex<i64>>,

    /// Watch channel for stats: late subscribers always receive the latest value.
    /// Ideal for derived/aggregate state — no events are ever missed.
    pub stats_tx: watch::Sender<TaskStats>,
    pub stats_rx: watch::Receiver<TaskStats>,

    /// Broadcast channel for task list: delivers a full snapshot after each mutation.
    /// Pre-serialized as JSON for zero-copy forwarding to SSE clients.
    pub list_tx: broadcast::Sender<serde_json::Value>,
}

impl AppState {
    pub fn new() -> Self {
        let initial_tasks = vec![
            Task {
                id: 1,
                title: "Learn Axum".into(),
                completed: true,
            },
            Task {
                id: 2,
                title: "Build Pilcrow".into(),
                completed: true,
            },
            Task {
                id: 3,
                title: "Ship feat/sse-hardening".into(),
                completed: false,
            },
            Task {
                id: 4,
                title: "Write the SSE example".into(),
                completed: false,
            },
        ];
        let initial_stats = TaskStats::from_tasks(&initial_tasks);
        let (stats_tx, stats_rx) = watch::channel(initial_stats);
        let (list_tx, _) = broadcast::channel(64);

        Self {
            tasks: Arc::new(Mutex::new(initial_tasks)),
            next_id: Arc::new(Mutex::new(5)),
            stats_tx,
            stats_rx,
            list_tx,
        }
    }

    pub fn mutate<R>(&self, f: impl FnOnce(&mut Vec<Task>) -> R) -> R {
        let mut tasks = self.tasks.lock().unwrap();
        let result = f(&mut tasks);
        let stats = TaskStats::from_tasks(&tasks);
        let list_patch = serde_json::json!({ "tasks": *tasks });
        drop(tasks);
        let _ = self.stats_tx.send(stats);
        let _ = self.list_tx.send(list_patch);
        result
    }
}
