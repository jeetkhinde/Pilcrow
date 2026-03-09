use crate::examples::sse::task_manager::model::{Task, TaskStats};
use maud::{Markup, html};

/// Render the full task manager dashboard.
///
/// `events_path` is passed in from the handler via `EVENTS.path()` — the same
/// compile-time constant used for route registration and the `sse()` modifier.
/// This keeps all three in sync with zero string duplication.
pub fn render_dashboard(tasks: &[Task], events_path: &str) -> Markup {
    let stats = TaskStats::from_tasks(tasks);

    html! {
        div #dashboard class="dashboard" {
            h1 { "Task Manager — SSE" }

            // ── Live stats ──────────────────────────────────────────
            // s-live points to the unified events endpoint.
            // The server sends targeted patches to "#live-stats".
            div #live-stats class="task-stats" s-live=(events_path) {
                (render_stat("📡", "Total", stats.tasks.length, "tasks.length"))
                (render_stat("⏳", "Pending", stats.tasks.pending, "tasks.pending"))
                (render_stat("✅", "Done", stats.tasks.completed, "tasks.completed"))
            }

            // ── Add task form ────────────────────────────────────────
            form
                id="task-form"
                class="task-form"
                s-action="/examples/sse/tasks"
                POST
                s-target="#task-form"
                s-optimistic="reset"
            {
                input
                    id="task-input"
                    class="task-input"
                    type="text"
                    name="title"
                    placeholder="What needs to be done?"
                    autocomplete="off"
                    required {}
                button class="task-btn" type="submit" { "+ Add Task" }
            }

            // ── Task list ────────────────────────────────────────────
            // Same s-live URL as #live-stats above.
            // The Silcrow SSE hub opens exactly ONE EventSource for both elements.
            // The server sends targeted patches to "#task-list".
            div #task-list-container {
                ul
                    #task-list
                    class="task-list"
                    s-list="tasks"
                    s-template="task-tpl"
                    s-live=(events_path)
                {
                    @for task in tasks {
                        (render_task_item(task))
                    }
                }
                (render_task_template())
            }
        }
    }
}

fn render_stat(icon: &str, label: &str, value: usize, s_bind: &str) -> Markup {
    html! {
        div class="stat" {
            div class="stat-icon" { (icon) }
            div class="stat-details" {
                div class="stat-label" { (label) }
                div class="stat-value" s-bind=(s_bind) { (value) }
            }
        }
    }
}

fn render_task_item(task: &Task) -> Markup {
    html! {
        li class="task-item" s-key=(task.id) {
            div class="task-item-left" {
                input
                    type="checkbox"
                    class="task-checkbox"
                    s-bind=".completed:checked"
                    s-action={"/examples/sse/tasks/" (task.id) "/toggle"} PATCH
                    checked[task.completed];
                span s-bind=".title" { (task.title) }
            }
            div class="task-item-right" {
                button
                    type="button"
                    class="task-delete-btn"
                    s-action={"/examples/sse/tasks/" (task.id) "/delete"} DELETE
                { "✗" }
            }
        }
    }
}

fn render_task_template() -> Markup {
    html! {
        template id="task-tpl" {
            li class="task-item" s-key=".id" {
                div class="task-item-left" {
                    input
                        type="checkbox"
                        class="task-checkbox"
                        s-bind=".completed:checked"
                        s-action={"/examples/sse/tasks/{s-key}/toggle"} PATCH;
                    span class="task-title" s-bind=".title" {}
                }
                div class="task-item-right" {
                    button
                        type="button"
                        class="task-delete-btn"
                        s-action={"/examples/sse/tasks/{s-key}/delete"} DELETE
                    { "✗" }
                }
            }
        }
    }
}
