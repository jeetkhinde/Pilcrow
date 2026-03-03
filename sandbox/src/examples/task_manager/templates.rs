use super::models::Task;
use maud::{html, Markup};

pub fn render_task_dashboard(tasks: &[Task]) -> Markup {
    html! {
        div #dashboard class="dashboard" {
            h1 { "Task Manager" }
            form id="task-form" class="task-form" s-action="/examples/tasks" POST s-target="#task-list" s-html {
                input id="task-input" class="task-input" type="text" name="title" placeholder="What needs to be done?" required;
                button class="task-btn" type="submit" { "Add Task" }
            }
            div #task-list-container {
                (render_task_list(tasks))
            }
        }
    }
}

pub fn render_task_list(tasks: &[Task]) -> Markup {
    html! {
        ul #task-list class="task-list" {
            @if tasks.is_empty() {
                li class="task-empty" { "All caught up! Add a new task above." }
            }
            @for task in tasks {
                li class="task-item" {
                    div class="task-item-left" {
                        form s-action=(format!("/examples/tasks/{}/toggle", task.id)) POST s-target="#task-list" s-html class="task-item-form" {
                            input type="checkbox" class="task-checkbox" checked[task.completed];
                        }
                        span class=(if task.completed { "task-text-done" } else { "task-text-open" }) {
                            (task.title)
                        }
                    }
                    div class="task-item-right" {
                        button class="task-delete-btn" s-action=(format!("/examples/tasks/{}/delete", task.id)) DELETE s-target="#task-list" s-html {
                            "Delete"
                        }
                    }
                }
            }
        }
    }
}
