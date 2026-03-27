use super::models::Profile;
use maud::{Markup, html};

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
        form s-action="/examples/profile" POST {
            input type="text" name="name" value=(profile.name);
            button type="submit" { "Update Profile" }
        }
    }
}
