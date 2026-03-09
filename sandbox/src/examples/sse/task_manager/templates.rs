use crate::examples::sse::task_manager::model::{Task, TaskStats};
use maud::{Markup, html};

pub fn render_task_dashboard(tasks: &[Task]) -> Markup {
    let stats = TaskStats::from(tasks);

    html! {
        div #dashboard class="dashboard" {
            h1 { "Task Manager (SSE)" }

            // Live Stats bar updated via SSE
            div #live-stats class="task-stats" style="border-color: var(--pilcrow-accent);
                        background-color: rgba(69, 123, 157, 0.05);"
                        s-live="/examples/sse/tasks/events" {
                (render_stat_card("📡", "Live Total Tasks", stats.tasks.length, "tasks.length"))
                (render_stat_card("⏳", "Live Pending", stats.tasks.pending, "tasks.pending"))
                (render_stat_card("✨", "Live Completed", stats.tasks.completed, "tasks.completed"))
            }
            br;
            form id="task-form" class="task-form" s-action="/examples/sse/tasks" POST s-target="#task-list" {
                input id="task-input" class="task-input" type="text" name="title"
                      placeholder="What needs to be done?" autocomplete="off" required {}
                button class="task-btn" type="submit" { "+ Add Task" }
            }

            p class="task-note" {
                "Try appending " code { "#fail" } " to a task name to simulate a server error."
            }

             div #task-list-container {
                ul #task-list class="task-list" s-list="tasks" s-template="task-tpl"
                    s-live="/examples/sse/tasks/list-events" {
                    @for task in tasks {
                        li class="task-item" s-key=(task.id) {
                            div class="task-item-left" {
                                input type="checkbox"
                                    class="task-checkbox"
                                    s-bind=".completed:checked"
                                    s-action={"/examples/sse/tasks/" (task.id) "/toggle"} PATCH
                                    checked[task.completed];
                                span s-bind=".title" { (task.title) }
                            }
                            div class="task-item-right" {
                                button type="button" class="task-delete-btn"
                                    s-action={"/examples/sse/tasks/" (task.id) "/delete"} DELETE    { "✗" }
                            }
                        }
                    }
                }
                (render_task_template())
            }
            ul s-target="#tasks" s-each="item in items" {
                li s-text="item.name" {}
                div s-target="#stats" s-bind="ping" {}
            }
        }
    }
}

fn render_stat_card(icon: &str, label: &str, value: usize, s_bind: &str) -> Markup {
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

fn render_task_template() -> Markup {
    html! {
        template id="task-tpl" {
            li class="task-item" s-key=".id" {
                div class="task-item-left" {
                    input type="checkbox" class="task-checkbox"
                        s-action={"/examples/sse/tasks/{s-key}/toggle"} PATCH
                        s-bind=".completed:checked";
                        span class="task-title" s-bind=".title" {}
                }
                div class="task-item-right" {
                    button type="button" class="task-delete-btn" title="Delete task"
                        s-action={"/examples/sse/tasks/{s-key}/delete"} DELETE {
                         { "✗" }
                    }
                }
            }
        }
    }
}
