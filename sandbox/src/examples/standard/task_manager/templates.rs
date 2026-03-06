use crate::examples::standard::task_manager::model::{Task, TaskStats};
use maud::{Markup, html};

pub fn render_task_dashboard(tasks: &[Task]) -> Markup {
    let stats = TaskStats::from(tasks);

    html! {
        div #dashboard class="dashboard" {
            h1 { "Task Manager" }
            h2 style="font-size: 1.2rem; margin-top: 1.5rem; margin-bottom: 0.5rem; color: var(--pilcrow-text);" { "RESTful Stats" }

            // Normal non-SSE patch-only stats
            div #stats class="task-stats" {
                (render_stat_card("📊", "Total Tasks", stats.tasks.length, "tasks.length"))
                (render_stat_card("⏳", "Pending", stats.tasks.pending, "tasks.pending"))
                (render_stat_card("✨", "Completed", stats.tasks.completed, "tasks.completed"))
            }
            br;
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
