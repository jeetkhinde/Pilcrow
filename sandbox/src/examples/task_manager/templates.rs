use super::models::Task;
use maud::{html, Markup};

pub fn render_task_dashboard(tasks: &[Task]) -> Markup {
    // We serialize tasks here so they can be initially populated by Silcrow on load
    let tasks_json = serde_json::to_string(&tasks).unwrap_or_else(|_| "[]".to_string());

    html! {
        div #dashboard class="dashboard" {
            h1 { "Task Manager" }
            form id="task-form" class="task-form" s-action="/examples/tasks" POST s-target="#dashboard" {
                input id="task-input" class="task-input" type="text" name="title" placeholder="What needs to be done?" required;
                button class="task-btn" type="submit" { "Add Task" }
            }
            div #task-list-container {
                ul #task-list class="task-list" s-list="tasks" s-template="task-tpl" {}
                template id="task-tpl" {
                    li class="task-item" s-bind=".key" {
                        div class="task-item-left" {
                            input type="checkbox" class="task-checkbox" s-bind=".completed:checked" ;
                            span s-bind=".title" {}
                        }
                        div class="task-item-right" {
                            button type="button" class="task-delete-btn" {
                                "Delete"
                            }
                        }
                    }
        }
            }
    }
}
