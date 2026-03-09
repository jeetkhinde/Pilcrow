use maud::{DOCTYPE, Markup, PreEscaped, html};

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
                        .task-stats { display: flex; gap: 1.5rem; padding: 1.5rem; background: #f8fafc; border-radius: 12px; border: 1px solid #e2e8f0;}
                        .stat { display: flex; align-items: center; gap: 1rem; padding: 1rem 1.5rem; background: white; border-radius: 8px; box-shadow: 0 1px 3px rgba(0,0,0,0.05); flex: 1;}
                        .stat-icon { font-size: 1.5rem; padding: 0.5rem; border-radius: 50%; background: #f1f5f9;}
                        .stat-details { display: flex; flex-direction: column;}
                        .stat-value { font-size: 1.5rem; font-weight: 700; color: #0f172a; line-height: 1.2;}
                        .stat-label { font-size: 0.875rem; color: #64748b; font-weight: 500; text-transform: uppercase; letter-spacing: 0.05em;}
                        .stat-pending .stat-icon { background: #fff7ed; }
                        .stat-completed .stat-icon { background: #f0fdf4; }
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
                        });
                    "))
                }
            }
            body {
                (content)
            }
        }
    }
}
