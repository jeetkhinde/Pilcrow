use maud::{html, Markup, PreEscaped, DOCTYPE};

use crate::models::{Profile, Task};

/// Full-page HTML layout wrapping inner content.
pub fn layout(content: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html {
            head {
                (PreEscaped(pilcrow::assets::script_tag()))
                style {
                    (PreEscaped("
                        .dashboard { max-width: 600px; margin: 40px auto; font-family: sans-serif; padding: 20px; box-shadow: 0 4px 6px rgba(0,0,0,0.1); border-radius: 8px; background: white; }
                        .dashboard h1 { text-align: center; color: #333; }
                        .task-form { display: flex; gap: 8px; margin-bottom: 24px; }
                        .task-input { flex: 1; padding: 12px; border: 1px solid #ddd; border-radius: 4px; font-size: 16px; }
                        .task-btn { padding: 12px 24px; background: #007bff; color: white; border: none; border-radius: 4px; cursor: pointer; font-weight: bold; font-size: 16px; }
                        .task-list { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 8px; }
                        .task-empty { color: #888; text-align: center; padding: 20px; font-style: italic; }
                        .task-item { display: flex; align-items: center; justify-content: space-between; padding: 16px; background: #f8f9fa; border: 1px solid #eee; border-radius: 4px; transition: background 0.2s; }
                        .task-item-left { display: flex; align-items: center; gap: 16px; }
                        .task-item-form { margin: 0; display: flex; align-items: center; }
                        .task-checkbox { cursor: pointer; width: 20px; height: 20px; accent-color: #28a745; }
                        .task-text-done { text-decoration: line-through; color: #aaa; font-size: 18px; }
                        .task-text-open { color: #333; font-size: 18px; }
                        .task-item-right { display: flex; gap: 8px; }
                        .task-delete-btn { background: #dc3545; color: white; border: none; padding: 8px 12px; border-radius: 4px; cursor: pointer; transition: background 0.2s; }
                        .task-delete-btn:hover { background: #c82333; }
                    "))
                }
                script {
                    (PreEscaped("
                        document.addEventListener('DOMContentLoaded', () => {
                            if (window.Silcrow) {
                                window.Silcrow.onToast((msg, level) => {
                                    alert(`[${level.toUpperCase()}] ${msg}`);
                                });
                            }
                            document.addEventListener('task:created', () => {
                                const input = document.getElementById('task-input');
                                if (input) input.value = '';
                            });
                            document.addEventListener('toast', (e) => {
                                if (e.detail && e.detail.msg) {
                                  alert(`[${(e.detail.level || 'info').toUpperCase()}] ${e.detail.msg}`);
                                }
                            });
                            document.addEventListener('change', (e) => {
                                if (e.target.matches('.task-checkbox')) {
                                    e.target.form.requestSubmit();
                                }
                            });
                        });
                    "))
                }
            }
            body s-debug {
                (content)
            }
        }
    }
}

/// Renders the profile view with an edit form.
pub fn render_profile(profile: &Profile) -> Markup {
    html! {
        div #header {
            "Current Name: "
            span s-bind="name" { (profile.name) }
        }
        div #sidebar {
            "Sidebar loaded for ID: " (profile.id)
        }
        hr;
        form s-action="/profile" POST {
            input type="text" name="name" value=(profile.name);
            button type="submit" { "Update Profile" }
        }
    }
}

pub fn render_task_dashboard(tasks: &[Task]) -> Markup {
    html! {
        div #dashboard class="dashboard" {
            h1 { "Task Manager" }
            form id="task-form" class="task-form" s-action="/tasks" POST s-target="#task-list" s-html {
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
                        form s-action=(format!("/tasks/{}/toggle", task.id)) POST s-target="#task-list" s-html class="task-item-form" {
                            input type="checkbox" class="task-checkbox" checked[task.completed];
                        }
                        span class=(if task.completed { "task-text-done" } else { "task-text-open" }) {
                            (task.title)
                        }
                    }
                    div class="task-item-right" {
                        button class="task-delete-btn" s-action=(format!("/tasks/{}/delete", task.id)) DELETE s-target="#task-list" s-html {
                            "Delete"
                        }
                    }
                }
            }
        }
    }
}
