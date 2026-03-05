use super::models::Task;
use maud::{Markup, html};

pub fn render_task_dashboard(tasks: &[Task]) -> Markup {
    html! {
            div #dashboard class="dashboard" {

                h1 { "Task Manager" }
              div #stats class="task-stats" {
        // Total
        div class="stat stat-total" {
            span class="stat-icon" { "📊" }
            div class="stat-details" {
                span class="stat-value" s-bind="tasks.length" { (tasks.len()) }
                span class="stat-label" { "Total Tasks" }
            }
        }

        // Pending
        div class="stat stat-pending" {
            span class="stat-icon" { "⏳" }
            div class="stat-details" {
                span class="stat-value" s-bind="tasks.pending" { (tasks.iter().filter(|t| !t.completed).count()) }
                span class="stat-label" { "Pending" }
            }
        }

        // Completed
        div class="stat stat-completed" {
            span class="stat-icon" { "✨" }
            div class="stat-details" {
                span class="stat-value" s-bind="tasks.completed" { (tasks.iter().filter(|t| t.completed).count()) }
                span class="stat-label" { "Completed" }
            }
        }
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

fn render_task_template() -> Markup {
    html! {
        // 3. THE CLIENT-SIDE TEMPLATE
        // Used ONLY when new tasks are added via JSON responses.
        template id="task-tpl" {
            // Silcrow injects s-key automatically here on clone!
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
