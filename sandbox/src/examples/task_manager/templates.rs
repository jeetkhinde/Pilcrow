use super::standard::model::Task;
use maud::{Markup, html};

pub fn render_task_dashboard(tasks: &[Task]) -> Markup {
    let pending_count = tasks.iter().filter(|t| !t.completed).count();
    let completed_count = tasks.iter().filter(|t| t.completed).count();
    let total_count = tasks.len();

    html! {
        div #dashboard class="dashboard" {
            h1 { "Task Manager (Dual-Stat)" }

            h2 style="font-size: 1.2rem; margin-bottom: 0.5rem; color: var(--pilcrow-text);" { "Normal Stats (RESTful)" }
            div #stats class="task-stats" {
                (render_stat_card("📊", "Total Tasks", total_count, "tasks.length"))
                (render_stat_card("⏳", "Pending", pending_count, "tasks.pending"))
                (render_stat_card("✨", "Completed", completed_count, "tasks.completed"))
            }

            h2 style="font-size: 1.2rem; margin-top: 1.5rem; margin-bottom: 0.5rem; color: var(--pilcrow-text);" { "Live Stats (SSE)" }
            div #live-stats class="task-stats" style="border-color: var(--pilcrow-accent); background-color: rgba(69, 123, 157, 0.05);" s-live="/examples/tasks-sse/events" {
                (render_stat_card("📡", "Live Total", total_count, "tasks.length"))
                (render_stat_card("⏳", "Live Pending", pending_count, "tasks.pending"))
                (render_stat_card("✨", "Live Completed", completed_count, "tasks.completed"))
            }

            form id="task-form" class="task-form" s-action="/examples/tasks" POST s-target="#task-list" {
                input id="task-input" class="task-input" type="text" name="title"
                      placeholder="What needs to be done?" autocomplete="off" required;
                button class="task-btn" type="submit" {"Add Task"}
            }
            div #task-list-container {
                ul #task-list class="task-list" s-list="tasks" s-template="task-tpl" {
                    @for task in tasks {
                        li class="task-item" s-key=(task.id) {
                            div class="task-item-left" {
                                input type="checkbox"
                                    class="task-checkbox"
                                    s-bind=".completed:checked"
                                    s-action={"/examples/tasks/" (task.id) "/toggle"} PATCH
                                    checked[task.completed];
                                span s-bind=".title" { (task.title) }
                            }

                            div class="task-item-right" {
                                button type="button" class="task-delete-btn"
                                    s-action={"/examples/tasks/" (task.id) "/delete"} DELETE {
                                    "Delete"
                                }
                            }
                        }
                    }
                }

                (render_task_template())
            }
        }
    }
}

fn render_stat_card(icon: &str, label: &str, value: usize, s_bind: &str) -> Markup {
    html! {
        div class="stat" {
            span class="stat-icon" { (icon) }
            div class="stat-details" {
                span class="stat-value" s-bind=(s_bind) { (value) }
                span class="stat-label" { (label) }
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
                        s-action={"/examples/tasks/{s-key}/toggle"} PATCH
                        s-bind=".completed:checked";
                    span s-bind=".title" {}
                }
                div class="task-item-right" {
                    button type="button" class="task-delete-btn"
                        s-action={"/examples/tasks/{s-key}/delete"} DELETE {
                        "Delete"
                    }
                }
            }
        }
    }
}
