use maud::{html, Markup, PreEscaped, DOCTYPE};

use crate::models::Profile;

/// Full-page HTML layout wrapping inner content.
pub fn layout(content: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html {
            head {
                (PreEscaped(pilcrow::assets::script_tag()))
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
